use std::sync::{Arc, Mutex};

use candle_core::{Device, Error, Result, Tensor};
use candle_transformers::generation::LogitsProcessor;
use candle_transformers::models::quantized_llama::ModelWeights;
use tokenizers::Tokenizer;

use super::tokenizer::{detect_eos_token_id, load_tokenizer};
use super::InferenceBackend;

pub(crate) struct KazeModel {
    device: Device,
    base_weights: ModelWeights,
    tokenizer: Arc<Mutex<Tokenizer>>,
    model_bytes_len: usize,
    max_context_tokens: usize,
    max_new_tokens: usize,
    eos_token_id: Option<u32>,
}

impl KazeModel {
    pub(crate) fn load(bytes: &[u8], tokenizer_bytes: Option<&[u8]>) -> Result<Self> {
        if bytes.is_empty() {
            return Err(Error::Msg("model bytes are empty".into()));
        }
        let device = Device::Cpu;
        let mut cursor = std::io::Cursor::new(bytes);
        let content = candle_core::quantized::gguf_file::Content::read(&mut cursor)?;
        match content.metadata.get("general.architecture") {
            Some(candle_core::quantized::gguf_file::Value::String(architecture))
                if architecture == "llama" => {}
            _ => {
                return Err(Error::Msg(
                    "only the llama GGUF architecture is supported by this backend".into(),
                ))
            }
        }
        let context_limit = match content.metadata.get("llama.context_length") {
            Some(value) => value.to_u32()? as usize,
            None => 2048,
        }
        .min(2048);
        if context_limit < 2 {
            return Err(Error::Msg(
                "GGUF context length must allow prompt and output tokens".into(),
            ));
        }
        let tokenizer = load_tokenizer(&content.metadata, tokenizer_bytes)?;
        let embedding = content
            .tensor_infos
            .get("token_embd.weight")
            .ok_or_else(|| Error::Msg("GGUF token_embd.weight is missing".into()))?;
        let vocab_size = tokenizer.get_vocab_size(true);
        if embedding.shape.dims().len() != 2 || embedding.shape.dims()[0] != vocab_size {
            return Err(Error::Msg(
                "GGUF embedding rows do not match the tokenizer vocabulary".into(),
            ));
        }
        let tokenizer = Arc::new(Mutex::new(tokenizer));
        let eos_token_id = detect_eos_token_id(&content.metadata, &tokenizer)?;
        let weights = ModelWeights::from_gguf(content, &mut cursor, &device)?;
        Ok(Self {
            device,
            base_weights: weights,
            tokenizer,
            model_bytes_len: bytes.len(),
            max_context_tokens: context_limit,
            max_new_tokens: 128,
            eos_token_id,
        })
    }
}

impl InferenceBackend for KazeModel {
    fn synthesize(&mut self, prompt: &str) -> Result<String> {
        let prompt = prompt.trim();
        if prompt.is_empty() {
            return Err(Error::Msg("synthesis prompt is empty".into()));
        }
        let mut all_tokens = self
            .tokenizer
            .lock()
            .map_err(|_| Error::Msg("tokenizer mutex poisoned".into()))?
            .encode(prompt, true)
            .map_err(|err| Error::Msg(format!("tokenizer encode error: {err}")))?
            .get_ids()
            .to_vec();
        if all_tokens.is_empty() {
            return Err(Error::Msg("tokenizer produced an empty prompt".into()));
        }
        if all_tokens.len() >= self.max_context_tokens {
            return Err(Error::Msg(format!(
                "prompt exceeds context budget ({} tokens; limit {})",
                all_tokens.len(),
                self.max_context_tokens
            )));
        }
        let prompt_len = all_tokens.len();
        let generation_budget = self
            .max_new_tokens
            .min(self.max_context_tokens - prompt_len);
        // Clone pristine weights so each request starts with an empty KV cache.
        let mut model = self.base_weights.clone();
        let mut sampler = LogitsProcessor::new(42, Some(0.8), Some(0.95));
        for step in 0..generation_budget {
            let input = if step == 0 {
                all_tokens.as_slice()
            } else {
                &all_tokens[all_tokens.len() - 1..]
            };
            let index_pos = all_tokens.len() - input.len();
            let tensor = Tensor::new(input, &self.device)?.unsqueeze(0)?;
            let logits = model.forward(&tensor, index_pos)?.squeeze(0)?;
            let next = sampler.sample(&logits)?;
            if Some(next) == self.eos_token_id {
                break;
            }
            all_tokens.push(next);
        }
        // Decode once: decoded prefixes are not guaranteed to be append-only.
        self.tokenizer
            .lock()
            .map_err(|_| Error::Msg("tokenizer mutex poisoned".into()))?
            .decode(&all_tokens[prompt_len..], true)
            .map(|text| text.trim().to_string())
            .map_err(|err| Error::Msg(format!("tokenizer decode error: {err}")))
    }

    fn tokenizer(&self) -> Arc<Mutex<Tokenizer>> {
        self.tokenizer.clone()
    }
    fn model_size(&self) -> usize {
        self.model_bytes_len
    }
    fn is_cpu_device(&self) -> bool {
        matches!(self.device, Device::Cpu)
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
    pub(crate) struct MockModel {
        tokenizer: Arc<Mutex<Tokenizer>>,
        model_bytes_len: usize,
    }

    impl MockModel {
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
pub(crate) use runtime_mock::MockModel;

#[cfg(any(test, feature = "mock_inference"))]
impl InferenceBackend for MockModel {
    fn synthesize(&mut self, prompt: &str) -> Result<String> {
        MockModel::synthesize(self, prompt)
    }
    fn tokenizer(&self) -> Arc<Mutex<Tokenizer>> {
        MockModel::tokenizer(self)
    }
    fn model_size(&self) -> usize {
        MockModel::model_size(self)
    }
    fn is_cpu_device(&self) -> bool {
        MockModel::is_cpu_device(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::quantized::{gguf_file, GgmlDType, QTensor};
    use candle_core::DType;

    // Synthetic, one-layer LLaMA GGUF: it exercises real quantized CPU forward,
    // KV cache, EOS and decode, but is not a trained language-quality benchmark.
    fn fixture(architecture: &str, embedded_json: bool) -> (Vec<u8>, Vec<u8>) {
        let (mut metadata, tokenizer) = super::super::tokenizer::tests::fixture();
        use gguf_file::Value;
        metadata.insert(
            "general.architecture".into(),
            Value::String(architecture.into()),
        );
        for (key, value) in [
            ("llama.attention.head_count", 1),
            ("llama.attention.head_count_kv", 1),
            ("llama.block_count", 1),
            ("llama.embedding_length", 32),
            ("llama.rope.dimension_count", 32),
        ] {
            metadata.insert(key.into(), Value::U32(value));
        }
        metadata.insert(
            "llama.attention.layer_norm_rms_epsilon".into(),
            Value::F32(1e-5),
        );
        if embedded_json {
            metadata.insert(
                "tokenizer.json".into(),
                Value::String(String::from_utf8(tokenizer.clone()).unwrap()),
            );
        }
        let cpu = Device::Cpu;
        let mut tensors: Vec<(String, QTensor)> = Vec::new();
        let mut add = |name: &str, tensor: Tensor, dtype| {
            tensors.push((name.to_string(), QTensor::quantize(&tensor, dtype).unwrap()));
        };
        add(
            "token_embd.weight",
            Tensor::ones((5, 32), DType::F32, &cpu).unwrap(),
            GgmlDType::Q4_0,
        );
        add(
            "output_norm.weight",
            Tensor::ones(32, DType::F32, &cpu).unwrap(),
            GgmlDType::F32,
        );
        let mut output = vec![0f32; 5 * 32];
        output[3 * 32..4 * 32].fill(1.0);
        add(
            "output.weight",
            Tensor::from_vec(output, (5, 32), &cpu).unwrap(),
            GgmlDType::Q4_0,
        );
        for suffix in [
            "attn_q",
            "attn_k",
            "attn_v",
            "attn_output",
            "ffn_gate",
            "ffn_down",
            "ffn_up",
        ] {
            add(
                &format!("blk.0.{suffix}.weight"),
                Tensor::zeros((32, 32), DType::F32, &cpu).unwrap(),
                GgmlDType::Q4_0,
            );
        }
        for suffix in ["attn_norm", "ffn_norm"] {
            add(
                &format!("blk.0.{suffix}.weight"),
                Tensor::ones(32, DType::F32, &cpu).unwrap(),
                GgmlDType::F32,
            );
        }
        let metadata_refs = metadata
            .iter()
            .map(|(key, value)| (key.as_str(), value))
            .collect::<Vec<_>>();
        let tensor_refs = tensors
            .iter()
            .map(|(name, tensor)| (name.as_str(), tensor))
            .collect::<Vec<_>>();
        let mut file = std::io::Cursor::new(Vec::new());
        gguf_file::write(&mut file, &metadata_refs, &tensor_refs).unwrap();
        (file.into_inner(), tokenizer)
    }

    #[test]
    fn quantized_cpu_forward_is_repeatable_with_fresh_kv_cache() {
        let (bytes, tokenizer) = fixture("llama", false);
        let mut model = KazeModel::load(&bytes, Some(&tokenizer)).unwrap();
        model.max_new_tokens = 3;
        assert!(model.is_cpu_device());
        let first = model.synthesize("東京 自然。").unwrap();
        assert_eq!(first, "自然 自然 自然");
        assert_eq!(model.synthesize("東京 自然。").unwrap(), first);
        model.eos_token_id = Some(3);
        assert_eq!(model.synthesize("東京 自然。").unwrap(), "");
    }

    #[test]
    fn prompt_and_generation_obey_context_budget_without_silent_truncation() {
        let (bytes, _) = fixture("llama", true);
        let mut model = KazeModel::load(&bytes, None).unwrap();
        model.max_context_tokens = 4;
        model.max_new_tokens = 128;
        assert_eq!(model.synthesize("東京 自然。").unwrap(), "自然");
        let error = model.synthesize("東京 東京 自然。").unwrap_err();
        assert!(error.to_string().contains("context budget"));
        assert_eq!(model.synthesize("東京 自然。").unwrap(), "自然");
    }

    #[test]
    fn gguf_load_fails_for_missing_tokenizer_or_unsupported_architecture() {
        let (bytes, tokenizer) = fixture("llama", false);
        assert!(KazeModel::load(&bytes, None).is_err());
        let (unsupported, _) = fixture("qwen2", false);
        assert!(KazeModel::load(&unsupported, Some(&tokenizer)).is_err());
        assert!(KazeModel::load(b"not gguf", Some(&tokenizer)).is_err());
    }
}
