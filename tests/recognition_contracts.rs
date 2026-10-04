//! Public model-free API also works without the default Sudachi feature.
use kaze_nhanh::*;
use std::sync::Arc;
struct EmptyAnalyzer;
impl MorphAnalyzer for EmptyAnalyzer {
    fn analyze(&self, _: &str) -> Result<MorphAnalysis, EvaluationError> {
        Ok(MorphAnalysis {
            morphemes: vec![],
            provenance: vec![],
        })
    }
}
#[test]
fn recognition_facade_without_assets_retains_uncertainty_and_raw_source_data() {
    let engine =
        RecognitionEngine::new(Arc::new(EmptyAnalyzer), RecognitionConfig::default()).unwrap();
    let text = "今日は晴れです";
    let annotations = [SourceAnnotation {
        span: ByteSpan::whole(text),
        data: serde_json::json!({"confidence":0.999}),
    }];
    let mut input = RecognitionInput::new(text, RecognitionSource::Asr, "session-1", "utterance-1");
    input.annotations = &annotations;
    let report = engine.evaluate_recognition(input).unwrap();
    assert_eq!(report.original_text, text);
    assert_eq!(report.annotations, annotations);
    assert_eq!(report.decision, RecognitionDecision::Undetermined);
    assert!(report.recognition_risk.value.is_none());
    assert!(report.recognition_risk.method.is_none());
    assert!(report.transcription_policy_id.is_none());
    assert_eq!(report.metrics.slm_calls, 0);
    let json = serde_json::to_string(&report).unwrap();
    let decoded: RecognitionReport = serde_json::from_str(&json).unwrap();
    decoded.validate().unwrap();
}
