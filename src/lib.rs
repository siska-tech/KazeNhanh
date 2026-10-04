//! Detection-first text evaluation facade. Git and generation are opt-in legacy APIs.
pub use kaze_nhanh_core::*;
#[cfg(feature = "sudachi")]
pub use kaze_nhanh_sudachi::{SudachiAnalyzer, SudachiConfig, SudachiMode};

/// Construct a local Japanese analyzer without any model or inference runtime.
#[cfg(feature = "sudachi")]
pub fn japanese_engine(
    config: SudachiConfig,
    evaluation: EvaluationConfig,
) -> Result<EvaluationEngine, EvaluationError> {
    EvaluationEngine::new(
        std::sync::Arc::new(SudachiAnalyzer::new(config)?),
        evaluation,
    )
}

/// Explicit 0.1 compatibility API. Evaluation errors contain no legacy types.
#[cfg(feature = "legacy")]
pub use kaze_nhanh_legacy::*;
