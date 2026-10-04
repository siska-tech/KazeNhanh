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
    let profile = DomainProfile::builtin(&evaluation.profile_id)?;
    japanese_engine_with_profile(config, profile, evaluation)
}

/// Explicit 0.1 compatibility API. Evaluation errors contain no legacy types.
#[cfg(feature = "legacy")]
pub use kaze_nhanh_legacy::*;

/// Assemble an explicitly versioned custom primary profile; no SLM is loaded.
#[cfg(feature = "sudachi")]
pub fn japanese_engine_with_profile(
    config: SudachiConfig,
    profile: DomainProfile,
    evaluation: EvaluationConfig,
) -> Result<EvaluationEngine, EvaluationError> {
    if profile.id != evaluation.profile_id {
        return Err(EvaluationError::InvalidConfig(
            "profile identifier mismatch".into(),
        ));
    }
    let rules = PrimaryRules::new(profile)?;
    Ok(EvaluationEngine::new(
        std::sync::Arc::new(SudachiAnalyzer::new(config)?),
        evaluation,
    )?
    .with_primary_detector(std::sync::Arc::new(rules)))
}

#[cfg(feature = "qwen")]
pub use kaze_nhanh_qwen::{QwenJudgeConfig, QwenNaturalnessFactory};
