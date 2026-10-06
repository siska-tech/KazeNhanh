//! Public evaluation API runs without loading model or dictionary resources.
use kaze_nhanh::{
    EvaluationConfig, EvaluationEngine, EvaluationError, MorphAnalysis, MorphAnalyzer, TextInput,
    Verdict,
};
use std::sync::Arc;
struct NoAssets;
impl MorphAnalyzer for NoAssets {
    fn analyze(&self, _: &str) -> Result<MorphAnalysis, EvaluationError> {
        Ok(MorphAnalysis::default())
    }
}
#[test]
fn public_evaluation_facade_needs_no_static_assets() {
    let engine = EvaluationEngine::new(Arc::new(NoAssets), EvaluationConfig::default()).unwrap();
    let input = "  入力フォーム🙂\n";
    let report = engine.evaluate(TextInput::new(input)).unwrap();
    assert_eq!(report.original_text, input);
    assert_eq!(report.verdict, Verdict::Undetermined);
    assert!(report.scores.validity.value.is_none());
    assert_eq!(report.metrics.slm_calls, 0);
}
