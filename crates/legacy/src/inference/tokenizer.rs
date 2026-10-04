use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use candle_core::quantized::gguf_file::Value;
use candle_core::{Error, Result};
use tokenizers::Tokenizer;

/// Standard GGUF vocabulary arrays are not serialized tokenizer JSON.
/// Require a full tokenizer and check every ID before executing model weights.
pub(crate) fn load_tokenizer(
    metadata: &HashMap<String, Value>,
    explicit_bytes: Option<&[u8]>,
) -> Result<Tokenizer> {
    let embedded;
    let bytes = match explicit_bytes {
        Some(bytes) => bytes,
        None => {
            embedded = match metadata.get("tokenizer.json") {
                Some(Value::String(json)) => json.as_bytes().to_vec(),
                Some(Value::Array(values)) => values.iter().map(|value| match value {
                    Value::U8(byte) => Ok(*byte),
                    _ => Err(Error::Msg("tokenizer.json byte array must contain U8 values".into())),
                }).collect::<Result<Vec<_>>>()?,
                Some(_) => return Err(Error::Msg("unsupported tokenizer.json metadata type".into())),
                None => return Err(Error::Msg(
                    "full tokenizer JSON is required; provide new_with_tokenizer or embed tokenizer.json (GGUF token arrays alone are unsupported)".into()
                )),
            };
            embedded.as_slice()
        }
    };
    let tokenizer = Tokenizer::from_bytes(bytes)
        .map_err(|err| Error::Msg(format!("failed to restore tokenizer: {err}")))?;
    let tokens = match metadata.get("tokenizer.ggml.tokens") {
        Some(Value::Array(tokens)) if !tokens.is_empty() => tokens,
        _ => {
            return Err(Error::Msg(
                "GGUF tokenizer.ggml.tokens must be a non-empty string array".into(),
            ))
        }
    };
    let vocab = tokenizer.get_vocab(true);
    if vocab.len() != tokens.len() {
        return Err(Error::Msg(format!(
            "tokenizer vocabulary size mismatch: JSON={}, GGUF={}",
            vocab.len(),
            tokens.len()
        )));
    }
    for (id, token) in tokens.iter().enumerate() {
        let Value::String(token) = token else {
            return Err(Error::Msg(
                "GGUF tokenizer.ggml.tokens must contain strings".into(),
            ));
        };
        if vocab.get(token).copied() != Some(id as u32) {
            return Err(Error::Msg(format!(
                "tokenizer vocabulary ID mismatch at {id}"
            )));
        }
    }
    Ok(tokenizer)
}

pub(crate) fn detect_eos_token_id(
    metadata: &HashMap<String, Value>,
    tokenizer: &Arc<Mutex<Tokenizer>>,
) -> Result<Option<u32>> {
    let tokenizer = tokenizer
        .lock()
        .map_err(|_| Error::Msg("tokenizer mutex poisoned".into()))?;
    let vocab = tokenizer.get_vocab(true);
    if let Some(value) = metadata.get("tokenizer.ggml.eos_token_id") {
        let id = value.to_u32()?;
        if !vocab.values().any(|candidate| *candidate == id) {
            return Err(Error::Msg(
                "GGUF EOS token ID is outside the tokenizer vocabulary".into(),
            ));
        }
        if let Some(value) = metadata.get("tokenizer.ggml.eos_token") {
            let Value::String(token) = value else {
                return Err(Error::Msg("GGUF EOS token must be a string".into()));
            };
            if vocab.get(token).copied() != Some(id) {
                return Err(Error::Msg("GGUF EOS token and ID disagree".into()));
            }
        }
        return Ok(Some(id));
    }
    if let Some(value) = metadata.get("tokenizer.ggml.eos_token") {
        let Value::String(token) = value else {
            return Err(Error::Msg("GGUF EOS token must be a string".into()));
        };
        return vocab.get(token).copied().map(Some).ok_or_else(|| {
            Error::Msg("GGUF EOS token is missing from the tokenizer vocabulary".into())
        });
    }
    Ok(["</s>", "<|eot_id|>", "<|endoftext|>"]
        .iter()
        .find_map(|token| vocab.get(*token).copied()))
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use tokenizers::models::wordlevel::WordLevelBuilder;
    use tokenizers::pre_tokenizers::whitespace::Whitespace;

    pub(crate) fn fixture() -> (HashMap<String, Value>, Vec<u8>) {
        let tokens = ["[UNK]", "</s>", "東京", "自然", "。"];
        let vocab = tokens
            .iter()
            .enumerate()
            .map(|(id, token)| (token.to_string(), id as u32))
            .collect();
        let model = WordLevelBuilder::default()
            .vocab(vocab)
            .unk_token("[UNK]".into())
            .build()
            .unwrap();
        let mut tokenizer = Tokenizer::new(model);
        tokenizer.with_pre_tokenizer(Whitespace::default());
        let bytes = tokenizer.to_string(false).unwrap().into_bytes();
        let mut metadata = HashMap::new();
        metadata.insert(
            "tokenizer.ggml.tokens".into(),
            Value::Array(
                tokens
                    .iter()
                    .map(|token| Value::String(token.to_string()))
                    .collect(),
            ),
        );
        metadata.insert("tokenizer.ggml.eos_token_id".into(), Value::U32(1));
        (metadata, bytes)
    }

    #[test]
    fn japanese_token_ids_match_external_and_embedded_json() {
        let (mut metadata, bytes) = fixture();
        let external = load_tokenizer(&metadata, Some(&bytes)).unwrap();
        assert_eq!(
            external.encode("東京 自然。", true).unwrap().get_ids(),
            &[2, 3, 4]
        );
        metadata.insert(
            "tokenizer.json".into(),
            Value::String(String::from_utf8(bytes.clone()).unwrap()),
        );
        let embedded = load_tokenizer(&metadata, None).unwrap();
        assert_eq!(
            embedded.encode("東京 自然。", true).unwrap().get_ids(),
            &[2, 3, 4]
        );
        metadata.insert(
            "tokenizer.json".into(),
            Value::Array(bytes.iter().map(|byte| Value::U8(*byte)).collect()),
        );
        assert_eq!(
            load_tokenizer(&metadata, None)
                .unwrap()
                .decode(&[2, 3, 4], true)
                .unwrap(),
            "東京 自然 。"
        );
    }

    #[test]
    fn missing_or_invalid_json_is_rejected_without_fallback() {
        let (mut metadata, _) = fixture();
        assert!(load_tokenizer(&metadata, None).is_err());
        assert!(load_tokenizer(&metadata, Some(b"invalid")).is_err());
        metadata.insert("tokenizer.json".into(), Value::Array(vec![Value::I8(-1)]));
        assert!(load_tokenizer(&metadata, None).is_err());
    }

    #[test]
    fn vocabulary_size_type_and_id_mismatches_are_rejected() {
        let (metadata, bytes) = fixture();
        let mut bad = metadata.clone();
        bad.insert(
            "tokenizer.ggml.tokens".into(),
            Value::Array(vec![Value::String("[UNK]".into())]),
        );
        assert!(load_tokenizer(&bad, Some(&bytes)).is_err());
        let mut bad = metadata.clone();
        let Value::Array(tokens) = bad.get_mut("tokenizer.ggml.tokens").unwrap() else {
            unreachable!()
        };
        tokens.swap(2, 3);
        assert!(load_tokenizer(&bad, Some(&bytes)).is_err());
        let Value::Array(tokens) = bad.get_mut("tokenizer.ggml.tokens").unwrap() else {
            unreachable!()
        };
        tokens[2] = Value::U8(1);
        assert!(load_tokenizer(&bad, Some(&bytes)).is_err());
    }

    #[test]
    fn eos_metadata_is_validated() {
        let (mut metadata, bytes) = fixture();
        let tokenizer = Arc::new(Mutex::new(load_tokenizer(&metadata, Some(&bytes)).unwrap()));
        assert_eq!(detect_eos_token_id(&metadata, &tokenizer).unwrap(), Some(1));
        metadata.insert("tokenizer.ggml.eos_token_id".into(), Value::U32(99));
        assert!(detect_eos_token_id(&metadata, &tokenizer).is_err());
        metadata.insert("tokenizer.ggml.eos_token_id".into(), Value::U32(1));
        metadata.insert(
            "tokenizer.ggml.eos_token".into(),
            Value::String("自然".into()),
        );
        assert!(detect_eos_token_id(&metadata, &tokenizer).is_err());
    }
}
