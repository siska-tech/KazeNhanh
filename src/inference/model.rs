#[cfg(not(any(test, feature = "mock_inference")))]
mod runtime {
    use std::fmt;
    use std::sync::{Arc, Mutex, MutexGuard};

    use candle_core::{Device, Error as CandleError, Result as CandleResult, Tensor};
    use candle_transformers::generation::LogitsProcessor;
    use candle_transformers::models::quantized_llama::ModelWeights;
    use tokenizers::Tokenizer;
    use tracing::warn;

    use super::tokenizer_support::{detect_eos_token_id, load_tokenizer};

    const DEFAULT_MAX_CONTEXT_TOKENS: usize = 2048;
    const DEFAULT_MAX_NEW_TOKENS: usize = 128;
    const DEFAULT_SAMPLING_SEED: u64 = 42;
    const DEFAULT_TEMPERATURE: f64 = 0.8;
    const DEFAULT_TOP_P: f64 = 0.95;

    #[derive(Clone)]
    pub(crate) struct KazeModel {
        device: Device,
        base_weights: ModelWeights,
        tokenizer: Arc<Mutex<Tokenizer>>,
        model_bytes_len: usize,
        max_context_tokens: usize,
        max_new_tokens: usize,
        sampling_seed: u64,
        temperature: Option<f64>,
        top_p: Option<f64>,
        eos_token_id: Option<u32>,
    }

    impl KazeModel {
        pub(crate) fn load(bytes: &[u8]) -> CandleResult<Self> {
            if bytes.is_empty() {
                return Err(CandleError::Msg("model bytes are empty".into()));
            }

            let model_bytes_len = bytes.len();
            let device = Device::Cpu;
            let mut cursor = std::io::Cursor::new(bytes);
            let content = candle_core::quantized::gguf_file::Content::read(&mut cursor)?;
            let tokenizer = Arc::new(Mutex::new(load_tokenizer(&content.metadata)?));
            let eos_token_id = detect_eos_token_id(&content.metadata, &tokenizer)?;

            let mut reader = &mut cursor;
            let model = ModelWeights::from_gguf(content, &mut reader, &device)?;

            Ok(Self {
                device,
                base_weights: model,
                tokenizer,
                model_bytes_len,
                max_context_tokens: DEFAULT_MAX_CONTEXT_TOKENS,
                max_new_tokens: DEFAULT_MAX_NEW_TOKENS,
                sampling_seed: DEFAULT_SAMPLING_SEED,
                temperature: Some(DEFAULT_TEMPERATURE),
                top_p: Some(DEFAULT_TOP_P),
                eos_token_id,
            })
        }

        pub(crate) fn ensure_cpu_device(&self) -> CandleResult<()> {
            if matches!(self.device, Device::Cpu) {
                Ok(())
            } else {
                Err(stage_error(
                    "device",
                    "only cpu inference is supported for quantized models",
                ))
            }
        }

        pub(crate) fn is_cpu_device(&self) -> bool {
            matches!(self.device, Device::Cpu)
        }

        pub(crate) fn synthesize(&mut self, prompt: &str) -> CandleResult<String> {
            let trimmed_prompt = prompt.trim();
            if trimmed_prompt.is_empty() {
                return Err(stage_error("prompt", "prompt is empty"));
            }

            let tokenizer_guard = self.lock_tokenizer()?;

            let encoding = tokenizer_guard
                .encode(trimmed_prompt, true)
                .map_err(|err| stage_error("tokenizer encode", err))?;
            let mut all_tokens = encoding.get_ids().to_vec();
            let prompt_token_count = all_tokens.len();
            let mut generated_tokens = Vec::new();
            let mut streamed_text = String::new();
            let mut last_decoded_len = 0usize;
            let mut decode_error_logged = false;

            if all_tokens.is_empty() {
                return Ok(String::new());
            }

            // Maintain a sliding window for KV cache context.
            let mut context_tokens = if all_tokens.len() > self.max_context_tokens {
                all_tokens
                    .iter()
                    .skip(all_tokens.len() - self.max_context_tokens)
                    .copied()
                    .collect::<Vec<_>>()
            } else {
                all_tokens.clone()
            };

            let mut model = self.base_weights.clone();
            let mut logits_processor =
                LogitsProcessor::new(self.sampling_seed, self.temperature, self.top_p);
            drop(tokenizer_guard);

            for step in 0..self.max_new_tokens {
                let input_slice = if step == 0 {
                    context_tokens.as_slice()
                } else {
                    let len = context_tokens.len();
                    &context_tokens[len - 1..]
                };

                let index_pos = all_tokens.len().saturating_sub(input_slice.len());

                let token_tensor = Tensor::new(input_slice, &self.device)
                    .map_err(|err| stage_error("tensor init", err))?
                    .unsqueeze(0)
                    .map_err(|err| stage_error("tensor expand", err))?;
                let logits = model
                    .forward(&token_tensor, index_pos)
                    .map_err(|err| stage_error("model forward", err))?
                    .squeeze(0)
                    .map_err(|err| stage_error("logits squeeze", err))?;
                let next_token = logits_processor
                    .sample(&logits)
                    .map_err(|err| stage_error("logits sampling", err))?;

                if self.eos_token_id == Some(next_token) {
                    break;
                }

                all_tokens.push(next_token);
                context_tokens.push(next_token);
                generated_tokens.push(next_token);

                if context_tokens.len() > self.max_context_tokens {
                    let overflow = context_tokens.len() - self.max_context_tokens;
                    context_tokens.drain(0..overflow);
                }

                let tokenizer_guard = self.lock_tokenizer()?;
                let decode_result = tokenizer_guard.decode(&generated_tokens, true);
                drop(tokenizer_guard);

                match decode_result {
                    Ok(decoded) => {
                        if decoded.len() > last_decoded_len {
                            if let Some(new_segment) = decoded.get(last_decoded_len..) {
                                streamed_text.push_str(new_segment);
                                last_decoded_len = decoded.len();
                            }
                        }
                    }
                    Err(err) => {
                        if !decode_error_logged {
                            warn!(target: "kaze_nhanh::inference", %err, "streaming decode failed; continuing");
                            decode_error_logged = true;
                        }
                    }
                }
            }

            let tokenizer_guard = self.lock_tokenizer()?;

            let output_tokens = &all_tokens[prompt_token_count..];
            if output_tokens.is_empty() {
                return Ok(String::new());
            }

            let decoded = tokenizer_guard
                .decode(output_tokens, true)
                .map_err(|err| stage_error("tokenizer decode", err))?;

            if decoded.len() > last_decoded_len {
                if let Some(new_segment) = decoded.get(last_decoded_len..) {
                    streamed_text.push_str(new_segment);
                }
            }

            Ok(streamed_text.trim().to_string())
        }

        pub(crate) fn tokenizer(&self) -> Arc<Mutex<Tokenizer>> {
            self.tokenizer.clone()
        }

        pub(crate) fn model_size(&self) -> usize {
            self.model_bytes_len
        }

        fn lock_tokenizer(&self) -> CandleResult<MutexGuard<'_, Tokenizer>> {
            self.tokenizer
                .lock()
                .map_err(|_| stage_error("tokenizer lock", "mutex poisoned"))
        }
    }

    fn stage_error(stage: &str, err: impl fmt::Display) -> CandleError {
        CandleError::Msg(format!("synthesis {stage} error: {err}"))
    }
}

#[cfg(not(any(test, feature = "mock_inference")))]
pub(crate) use runtime::KazeModel;

mod tokenizer_support {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use candle_core::quantized::gguf_file;
    use candle_core::{Error as CandleError, Result as CandleResult};
    use tokenizers::models::wordlevel::WordLevelBuilder;
    use tokenizers::pre_tokenizers::whitespace::Whitespace;
    use tokenizers::Tokenizer;

    pub(crate) fn load_tokenizer(
        metadata: &HashMap<String, gguf_file::Value>,
    ) -> CandleResult<Tokenizer> {
        if let Some(bytes) = find_tokenizer_bytes(metadata) {
            return Tokenizer::from_bytes(&bytes)
                .map_err(|err| CandleError::Msg(format!("failed to restore tokenizer: {err}")));
        }

        build_fallback_tokenizer()
    }

    #[cfg_attr(test, allow(dead_code))]
    pub(crate) fn detect_eos_token_id(
        metadata: &HashMap<String, gguf_file::Value>,
        tokenizer: &Arc<Mutex<Tokenizer>>,
    ) -> CandleResult<Option<u32>> {
        if let Some(value) = metadata.get("tokenizer.ggml.eos_token_id") {
            if let Ok(id) = value.to_u32() {
                return Ok(Some(id));
            }
        }

        if let Some(value) = metadata.get("tokenizer.ggml.eos_token") {
            if let gguf_file::Value::String(token) = value {
                if let Some(id) = lookup_token_id(tokenizer, token)? {
                    return Ok(Some(id));
                }
            }
        }

        for candidate in ["</s>", "<|eot_id|>", "<|endoftext|>"] {
            if let Some(id) = lookup_token_id(tokenizer, candidate)? {
                return Ok(Some(id));
            }
        }

        Ok(None)
    }

    #[cfg_attr(test, allow(dead_code))]
    fn lookup_token_id(
        tokenizer: &Arc<Mutex<Tokenizer>>,
        token: &str,
    ) -> CandleResult<Option<u32>> {
        let guard = tokenizer
            .lock()
            .map_err(|_| CandleError::Msg("failed to lock tokenizer".into()))?;
        Ok(guard.get_vocab(true).get(token).copied())
    }

    fn find_tokenizer_bytes(metadata: &HashMap<String, gguf_file::Value>) -> Option<Vec<u8>> {
        for key in ["tokenizer.json", "tokenizer.ggml.tokens"] {
            if let Some(value) = metadata.get(key) {
                if let Some(bytes) = value_to_bytes(value) {
                    return Some(bytes);
                }
            }
        }
        None
    }

    fn value_to_bytes(value: &gguf_file::Value) -> Option<Vec<u8>> {
        match value {
            gguf_file::Value::String(s) => Some(s.as_bytes().to_vec()),
            gguf_file::Value::Array(values) => {
                let mut bytes = Vec::with_capacity(values.len());
                for value in values {
                    match value {
                        gguf_file::Value::U8(v) => bytes.push(*v),
                        gguf_file::Value::I8(v) => bytes.push(*v as u8),
                        _ => return None,
                    }
                }
                Some(bytes)
            }
            _ => None,
        }
    }

    fn build_fallback_tokenizer() -> CandleResult<Tokenizer> {
        let mut vocab = HashMap::new();
        vocab.insert("[PAD]".to_string(), 0);
        vocab.insert("[UNK]".to_string(), 1);

        let model = WordLevelBuilder::default()
            .vocab(vocab)
            .unk_token("[UNK]".to_string())
            .build()
            .map_err(|err| {
                CandleError::Msg(format!("failed to build fallback tokenizer: {err}"))
            })?;

        let mut tokenizer = Tokenizer::new(model);
        tokenizer.with_pre_tokenizer(Whitespace::default());
        Ok(tokenizer)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::sync::{Arc, Mutex};

        fn assert_send_sync<T: Send + Sync>() {}

        #[test]
        fn load_tokenizer_returns_fallback_when_metadata_missing() {
            let metadata = HashMap::new();

            let tokenizer = load_tokenizer(&metadata).expect("fallback tokenizer should build");
            let encoding = tokenizer
                .encode("hello world", false)
                .expect("encoding should succeed");

            assert!(!encoding.get_ids().is_empty());
        }

        #[test]
        fn find_tokenizer_bytes_reads_string_and_arrays() {
            let mut metadata = HashMap::new();
            metadata.insert(
                "tokenizer.json".to_string(),
                gguf_file::Value::String("{\"dummy\":true}".to_string()),
            );

            let bytes = find_tokenizer_bytes(&metadata).expect("should extract string bytes");
            assert_eq!(bytes, b"{\"dummy\":true}");

            let mut metadata = HashMap::new();
            metadata.insert(
                "tokenizer.ggml.tokens".to_string(),
                gguf_file::Value::Array(vec![
                    gguf_file::Value::U8(b'a'),
                    gguf_file::Value::U8(b'b'),
                    gguf_file::Value::U8(b'c'),
                ]),
            );

            let bytes = find_tokenizer_bytes(&metadata).expect("should read byte array");
            assert_eq!(bytes, b"abc");
        }

        #[test]
        fn arc_mutex_tokenizer_is_send_and_sync() {
            assert_send_sync::<Arc<Mutex<Tokenizer>>>();
        }
    }
}

#[cfg(any(test, feature = "mock_inference"))]
mod runtime_mock {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use candle_core::Error as CandleError;
    use tokenizers::models::wordlevel::WordLevelBuilder;
    use tokenizers::pre_tokenizers::whitespace::Whitespace;
    use tokenizers::Tokenizer;

    #[derive(Clone)]
    pub(crate) struct KazeModel {
        tokenizer: Arc<Mutex<Tokenizer>>,
        model_bytes_len: usize,
    }

    impl KazeModel {
        pub(crate) fn load(bytes: &[u8]) -> Result<Self, CandleError> {
            if bytes.is_empty() {
                return Err(CandleError::Msg("model bytes are empty".into()));
            }

            let tokenizer = Arc::new(Mutex::new(build_mock_tokenizer()?));

            Ok(Self {
                tokenizer,
                model_bytes_len: bytes.len(),
            })
        }

        pub(crate) fn ensure_cpu_device(&self) -> Result<(), CandleError> {
            Ok(())
        }

        pub(crate) fn is_cpu_device(&self) -> bool {
            true
        }

        pub(crate) fn synthesize(&mut self, prompt: &str) -> Result<String, CandleError> {
            let trimmed = prompt.trim();
            if trimmed.eq_ignore_ascii_case("raise") {
                return Err(CandleError::Msg("mock inference failure".into()));
            }

            Ok(format!("[mock-bytes:{}] {}", self.model_bytes_len, trimmed))
        }

        pub(crate) fn tokenizer(&self) -> Arc<Mutex<Tokenizer>> {
            self.tokenizer.clone()
        }

        pub(crate) fn model_size(&self) -> usize {
            self.model_bytes_len
        }
    }

    fn build_mock_tokenizer() -> Result<Tokenizer, CandleError> {
        let mut vocab = HashMap::new();
        vocab.insert("[PAD]".to_string(), 0);
        vocab.insert("[UNK]".to_string(), 1);
        vocab.insert("raise".to_string(), 2);

        let model = WordLevelBuilder::default()
            .vocab(vocab)
            .unk_token("[UNK]".to_string())
            .build()
            .map_err(|err| CandleError::Msg(format!("failed to build mock tokenizer: {err}")))?;

        let mut tokenizer = Tokenizer::new(model);
        tokenizer.with_pre_tokenizer(Whitespace::default());
        Ok(tokenizer)
    }
}

#[cfg(any(test, feature = "mock_inference"))]
pub(crate) use runtime_mock::KazeModel;
