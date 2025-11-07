use std::sync::{Arc, Mutex};
use tokenizers::Tokenizer;

use crate::{EngineConfig, KazeNhanhError};

mod model;
use model::KazeModel;

pub(crate) struct InferenceEngine {
    model: KazeModel,
    tokenizer: Arc<Mutex<Tokenizer>>,
}

impl InferenceEngine {
    pub(crate) fn new(config: &EngineConfig) -> Result<Self, KazeNhanhError> {
        let model = KazeModel::load(config.model_bytes)
            .map_err(|source| KazeNhanhError::ModelLoadError { source })?;

        model
            .ensure_cpu_device()
            .map_err(|source| KazeNhanhError::ModelLoadError { source })?;

        let tokenizer = model.tokenizer();

        Ok(Self { model, tokenizer })
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
        self.tokenizer.clone()
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
    fn initializes_with_non_empty_model_bytes() {
        let config = EngineConfig::new(b"model", b"dict", br#"{}"#);

        let mut engine = InferenceEngine::new(&config).expect("should initialize");

        assert_eq!(engine.model_size(), 5);

        let output = engine
            .synthesize("summarize the latest changes")
            .expect("should synthesize");
        assert!(output.contains("summarize"));
    }

    #[test]
    fn synthesize_rejects_empty_prompt() {
        let config = EngineConfig::new(b"model", b"dict", br#"{}"#);

        let mut engine = InferenceEngine::new(&config).expect("should initialize");

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

        let mut engine = InferenceEngine::new(&config).expect("should initialize");

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
    fn synthesize_mock_latency_is_bounded() {
        let config = EngineConfig::new(b"model", b"dict", br#"{}"#);

        let mut engine = InferenceEngine::new(&config).expect("should initialize");

        let start = std::time::Instant::now();
        let output = engine
            .synthesize("latency measurement prompt")
            .expect("should synthesize");
        let elapsed = start.elapsed();

        eprintln!("mock_synthesize_latency_ns={}", elapsed.as_nanos());

        assert!(
            elapsed <= std::time::Duration::from_millis(5),
            "mock synthesize latency {:?} exceeded 5ms budget",
            elapsed
        );

        assert!(output.contains("latency measurement"));
    }

    #[test]
    fn tokenizer_is_shared_across_threads() {
        let config = EngineConfig::new(b"model", b"dict", br#"{}"#);

        let engine = InferenceEngine::new(&config).expect("should initialize");
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

        let engine = InferenceEngine::new(&config).expect("should initialize");

        assert!(engine.is_cpu_device());
    }
}
