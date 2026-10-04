use super::*;

struct IdentityAnalyzer;
impl MorphAnalyzer for IdentityAnalyzer {
    fn analyze(&self, text: &str) -> Result<MorphAnalysis, EvaluationError> {
        Ok(MorphAnalysis {
            morphemes: if text.is_empty() {
                vec![]
            } else {
                vec![Morpheme {
                    surface: text.into(),
                    span: ByteSpan::whole(text),
                    dictionary_form: text.into(),
                    normalized_form: text.into(),
                    reading: String::new(),
                    part_of_speech: vec!["fixture".into()],
                    is_oov: false,
                    dictionary_id: 0,
                    synonym_group_ids: vec![],
                    cumulative_cost: None,
                }]
            },
            provenance: vec![],
        })
    }
}
fn engine() -> RecognitionEngine {
    RecognitionEngine::new(Arc::new(IdentityAnalyzer), RecognitionConfig::default()).unwrap()
}
fn report(text: &str) -> RecognitionReport {
    engine()
        .evaluate_recognition(RecognitionInput::new(
            text,
            RecognitionSource::Ocr,
            "document",
            "segment",
        ))
        .unwrap()
}
#[test]
fn fluent_and_unflagged_recognition_results_abstain_for_both_sources() {
    for source in [RecognitionSource::Ocr, RecognitionSource::Asr] {
        for text in [
            "節約する",
            "体系キープ",
            "10日やせろ",
            "",
            "  cafe\u{301}🙂\r\n",
        ] {
            let result = engine()
                .evaluate_recognition(RecognitionInput::new(text, source, "d", "s"))
                .unwrap();
            assert_eq!(result.original_text, text);
            assert_eq!(result.decision, RecognitionDecision::Undetermined);
            assert_eq!(result.recognition_risk.value, None);
            assert_eq!(
                result.recognition_risk.status,
                RecognitionAssessmentStatus::InsufficientEvidence
            );
            assert_eq!(result.evidence_adequacy, EvidenceAdequacy::Limited);
            assert!(result.coverage.risk_assessed_spans.is_empty());
            assert_eq!(
                result.coverage.risk_unassessed_spans,
                vec![ByteSpan::whole(text)]
            );
            assert!(!result.coverage.source_completeness_assessed);
            assert_eq!(result.naturalness.value, None);
            assert_eq!(result.metrics.slm_calls, 0);
        }
    }
}
#[test]
fn high_raw_confidence_is_preserved_without_becoming_low_risk() {
    let text = "ムダ使いをーない";
    let annotations = [SourceAnnotation {
        span: ByteSpan::whole(text),
        data: serde_json::json!({"confidence":0.975,"engine":"PP-OCRv6 medium"}),
    }];
    let mut input = RecognitionInput::new(text, RecognitionSource::Ocr, "d", "s");
    input.annotations = &annotations;
    let result = engine().evaluate_recognition(input).unwrap();
    assert_eq!(result.annotations, annotations);
    assert_eq!(result.decision, RecognitionDecision::Undetermined);
    assert_eq!(result.recognition_risk.value, None);
    assert_eq!(
        result
            .evidence
            .iter()
            .find(|e| e.kind == RecognitionEvidenceKind::Recognizer)
            .unwrap()
            .status,
        RecognitionEvidenceStatus::Unsupported
    );
}
#[test]
fn anomalies_require_review_without_claiming_a_risk_probability() {
    let result = report("猫猫猫猫猫猫");
    assert_eq!(result.decision, RecognitionDecision::Review);
    assert_eq!(result.recognition_risk.value, None);
    assert_eq!(result.evidence_adequacy, EvidenceAdequacy::Limited);
    assert!(result
        .findings
        .iter()
        .any(|f| f.kind == RecognitionFindingKind::Anomaly));
    let mut forged = result;
    forged.decision = RecognitionDecision::Undetermined;
    assert!(forged.validate().is_err());
}
#[test]
fn input_constraints_are_distinct_from_recognition_anomalies() {
    let result = report("猫\u{0001}");
    assert_eq!(result.decision, RecognitionDecision::Review);
    assert!(result
        .findings
        .iter()
        .any(|f| f.kind == RecognitionFindingKind::InputConstraint
            && f.issue.code == "forbidden_control"));
    assert_eq!(result.recognition_risk.value, None);
}
#[test]
fn report_rejects_forged_low_risk_probability_coverage_and_missing_evidence() {
    let original = report("節約する");
    let mut forged = original.clone();
    forged.decision = RecognitionDecision::LowRisk;
    assert!(forged.validate().is_err());
    let mut forged = original.clone();
    forged.recognition_risk.value = Some(UnitScore::new(0.0).unwrap());
    assert!(forged.validate().is_err());
    forged.recognition_risk.status = RecognitionAssessmentStatus::Estimated;
    forged.recognition_risk.calibration_id = Some("calibration".into());
    assert!(forged.validate().is_err()); // No risk estimator or assessed risk coverage.
    let mut forged = original.clone();
    forged.coverage.risk_assessed_spans = vec![ByteSpan::whole(&forged.original_text)];
    assert!(forged.validate().is_err());
    let mut forged = original.clone();
    forged.evidence_adequacy = EvidenceAdequacy::Sufficient;
    assert!(forged.validate().is_err());
    let mut forged = original.clone();
    forged.evidence.pop();
    assert!(forged.validate().is_err());
    let mut forged = original;
    forged.evidence[0].kind = RecognitionEvidenceKind::TextRules;
    assert!(forged.validate().is_err());
}
#[test]
fn report_roundtrips_and_rejects_unknown_schema_fields_and_bad_spans() {
    let original = report("猫🙂");
    let value = serde_json::to_value(&original).unwrap();
    assert!(value.get("correction").is_none());
    assert!(value.get("verdict").is_none());
    let decoded: RecognitionReport = serde_json::from_value(value.clone()).unwrap();
    decoded.validate().unwrap();
    assert_eq!(decoded, original);
    let mut forged = value.clone();
    forged["schema_version"] = "kzn.evaluation.v3".into();
    assert!(serde_json::from_value::<RecognitionReport>(forged)
        .unwrap()
        .validate()
        .is_err());
    let mut forged = value.clone();
    forged["correction"] = "犬".into();
    assert!(serde_json::from_value::<RecognitionReport>(forged).is_err());
    let mut forged = value.clone();
    forged["evidence"][0]["span"]["end"] = 1.into();
    assert!(serde_json::from_value::<RecognitionReport>(forged)
        .unwrap()
        .validate()
        .is_err());
    let mut forged = value;
    forged["recognition_risk"]["value"] = 1.1.into();
    assert!(serde_json::from_value::<RecognitionReport>(forged).is_err());
}
#[test]
fn batch_preserves_order_and_isolates_invalid_input() {
    let result = engine().evaluate_recognition_batch(&[
        RecognitionInput::new("節約する", RecognitionSource::Ocr, "d", "s1"),
        RecognitionInput::new("猫", RecognitionSource::Asr, "", "s2"),
        RecognitionInput::new("健康的になる", RecognitionSource::Asr, "d", "s3"),
    ]);
    assert_eq!(result[0].as_ref().unwrap().segment_id, "s1");
    assert!(result[1].is_err());
    assert_eq!(result[2].as_ref().unwrap().segment_id, "s3");
    assert!(RecognitionEngine::new(
        Arc::new(IdentityAnalyzer),
        RecognitionConfig { max_input_bytes: 0 }
    )
    .is_err());
}
#[test]
fn invalid_annotation_is_rejected_before_backend_execution() {
    struct PanicAnalyzer;
    impl MorphAnalyzer for PanicAnalyzer {
        fn analyze(&self, _: &str) -> Result<MorphAnalysis, EvaluationError> {
            panic!("must not execute");
        }
    }
    let engine =
        RecognitionEngine::new(Arc::new(PanicAnalyzer), RecognitionConfig::default()).unwrap();
    let annotation: SourceAnnotation =
        serde_json::from_value(serde_json::json!({"span":{"start":0,"end":1},"data":{}})).unwrap();
    let mut input = RecognitionInput::new("猫", RecognitionSource::Ocr, "d", "s");
    input.annotations = std::slice::from_ref(&annotation);
    assert!(engine.evaluate_recognition(input).is_err());
}

#[test]
fn low_risk_contract_requires_calibrated_target_policy_coverage_and_threshold() {
    // Synthetic report for contract validation, not an accepted model or runtime result.
    let mut synthetic = report("節約する");
    synthetic.transcription_policy_id = Some("fixture.literal.v1".into());
    synthetic.recognition_risk.status = RecognitionAssessmentStatus::Estimated;
    synthetic.recognition_risk.method = Some(ScoreMethod::Calibrated);
    synthetic.recognition_risk.value = Some(UnitScore::new(0.1).unwrap());
    synthetic.recognition_risk.calibration_id = Some("fixture.calibration.v1".into());
    let estimator = synthetic
        .evidence
        .iter_mut()
        .find(|e| e.kind == RecognitionEvidenceKind::RiskEstimator)
        .unwrap();
    estimator.status = RecognitionEvidenceStatus::Observed;
    estimator.value = Some(serde_json::json!({"fixture":true}));
    synthetic.coverage.risk_assessed_spans = vec![ByteSpan::whole(&synthetic.original_text)];
    synthetic.coverage.risk_unassessed_spans.clear();
    synthetic.evidence_adequacy = EvidenceAdequacy::Sufficient;
    synthetic.decision_policy.low_risk_threshold = Some(UnitScore::new(0.2).unwrap());
    synthetic.decision_policy.calibration_id = synthetic.recognition_risk.calibration_id.clone();
    synthetic.decision = RecognitionDecision::LowRisk;
    synthetic.validate().unwrap();
    let mut forged = synthetic.clone();
    forged.recognition_risk.value = Some(UnitScore::new(0.3).unwrap());
    assert!(forged.validate().is_err());
    let mut forged = synthetic.clone();
    forged.decision_policy.calibration_id = Some("different.calibration".into());
    assert!(forged.validate().is_err());
    let mut forged = synthetic.clone();
    forged.evidence_adequacy = EvidenceAdequacy::Limited;
    assert!(forged.validate().is_err());
    let mut forged = synthetic.clone();
    forged.transcription_policy_id = None;
    assert!(forged.validate().is_err());
    let mut forged = synthetic;
    forged.recognition_risk.method = Some(ScoreMethod::Heuristic);
    assert!(forged.validate().is_err());
}
