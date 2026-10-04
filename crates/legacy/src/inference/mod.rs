use std::sync::{Arc, Mutex};
use tokenizers::Tokenizer;

use crate::{EngineConfig, KazeNhanhError};

mod model;
mod tokenizer;
use model::KazeModel;

/// Backends are injected explicitly; compiling tests never replaces the runtime.
pub(crate) trait InferenceBackend: Send {
    fn synthesize(&mut self, prompt: &str) -> candle_core::Result<String>;
    fn tokenizer(&self) -> Arc<Mutex<Tokenizer>>;
    fn model_size(&self) -> usize;
    fn is_cpu_device(&self) -> bool;
}

pub(crate) struct InferenceEngine {
    model: Box<dyn InferenceBackend>,
}

impl InferenceEngine {
    pub(crate) fn new(config: &EngineConfig) -> Result<Self, KazeNhanhError> {
        Self::new_with_tokenizer(config, None)
    }

    pub(crate) fn new_with_tokenizer(
        config: &EngineConfig,
        tokenizer_bytes: Option<&[u8]>,
    ) -> Result<Self, KazeNhanhError> {
        let model = KazeModel::load(config.model_bytes, tokenizer_bytes)
            .map_err(|source| KazeNhanhError::ModelLoadError { source })?;
        Ok(Self::with_backend(Box::new(model)))
    }

    pub(crate) fn with_backend(model: Box<dyn InferenceBackend>) -> Self {
        Self { model }
    }

    #[cfg(any(test, feature = "mock_inference"))]
    pub(crate) fn mock(bytes: &[u8]) -> Result<Self, KazeNhanhError> {
        let model = model::MockModel::load(bytes)
            .map_err(|source| KazeNhanhError::ModelLoadError { source })?;
        Ok(Self::with_backend(Box::new(model)))
    }

    pub(crate) fn synthesize(&mut self, prompt: &str) -> Result<String, KazeNhanhError> {
        if prompt.trim().is_empty() {
            return Err(KazeNhanhError::InvalidInput(
                "Prompt for inference is empty".to_string(),
            ));
        }
        self.model
            .synthesize(prompt)
            .map_err(|source| KazeNhanhError::ModelInferenceError { source })
    }

    #[allow(dead_code)]
    pub(crate) fn model_size(&self) -> usize {
        self.model.model_size()
    }

    #[allow(dead_code)]
    pub(crate) fn tokenizer(&self) -> Arc<Mutex<Tokenizer>> {
        self.model.tokenizer()
    }

    #[allow(dead_code)]
    pub(crate) fn is_cpu_device(&self) -> bool {
        self.model.is_cpu_device()
    }
}

#[cfg(test)]
mod tests {
    use super::InferenceEngine;
    use crate::{EngineConfig, KazeNhanhError};

    #[test]
    fn returns_model_load_error_when_bytes_missing() {
        let config = EngineConfig::new(b"", b"dict", br#"{}"#);

        let error = InferenceEngine::new(&config)
            .err()
            .expect("expected model load error");

        match error {
            KazeNhanhError::ModelLoadError { source } => {
                assert!(source.to_string().contains("empty"));
            }
            other => panic!("unexpected error variant: {:?}", other),
        }
    }

    #[test]
    fn injected_mock_synthesizes_with_non_empty_model_bytes() {
        let config = EngineConfig::new(b"model", b"dict", br#"{}"#);

        let mut engine = InferenceEngine::mock(config.model_bytes).expect("should initialize");

        assert_eq!(engine.model_size(), 5);

        let output = engine
            .synthesize("summarize the latest changes")
            .expect("should synthesize");
        assert!(output.contains("summarize"));
    }

    #[test]
    fn synthesize_rejects_empty_prompt() {
        let config = EngineConfig::new(b"model", b"dict", br#"{}"#);

        let mut engine = InferenceEngine::mock(config.model_bytes).expect("should initialize");

        let error = engine
            .synthesize("   ")
            .err()
            .expect("expected invalid input error");

        match error {
            KazeNhanhError::InvalidInput(message) => {
                assert!(message.contains("Prompt"));
            }
            other => panic!("unexpected error variant: {:?}", other),
        }
    }

    #[test]
    fn synthesize_maps_candle_error() {
        let config = EngineConfig::new(b"model", b"dict", br#"{}"#);

        let mut engine = InferenceEngine::mock(config.model_bytes).expect("should initialize");

        let error = engine
            .synthesize("raise")
            .err()
            .expect("expected model inference error");

        match error {
            KazeNhanhError::ModelInferenceError { source } => {
                assert!(source.to_string().contains("mock"));
            }
            other => panic!("unexpected error variant: {:?}", other),
        }
    }

    #[test]
    fn tokenizer_is_shared_across_threads() {
        let config = EngineConfig::new(b"model", b"dict", br#"{}"#);

        let engine = InferenceEngine::mock(config.model_bytes).expect("should initialize");
        let tokenizer = engine.tokenizer();

        let handles = (0..4)
            .map(|_| {
                let tokenizer = tokenizer.clone();
                std::thread::spawn(move || {
                    let guard = tokenizer.lock().expect("tokenizer mutex poisoned");
                    guard.encode("raise", false).expect("encode should succeed");
                })
            })
            .collect::<Vec<_>>();

        for handle in handles {
            handle.join().expect("thread should complete");
        }
    }

    #[test]
    fn constructor_forces_cpu_device() {
        let config = EngineConfig::new(b"model", b"dict", br#"{}"#);

        let engine = InferenceEngine::mock(config.model_bytes).expect("should initialize");

        assert!(engine.is_cpu_device());
    }
}
