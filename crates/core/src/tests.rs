use super::*;

struct IdentityAnalyzer;
impl MorphAnalyzer for IdentityAnalyzer {
    fn analyze(&self, text: &str) -> Result<MorphAnalysis, EvaluationError> {
        Ok(MorphAnalysis {
            morphemes: if text.is_empty() {
                vec![]
            } else {
                vec![Morpheme {
                    span: ByteSpan::whole(text),
                    surface: text.into(),
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
fn engine(config: EvaluationConfig) -> EvaluationEngine {
    EvaluationEngine::new(Arc::new(IdentityAnalyzer), config).unwrap()
}

#[test]
fn original_text_and_annotations_survive_without_normalization_or_splitting() {
    let text = " \r\n東京都🙂 cafe\u{301} ＡＢＣ 1.23 https://example.test/a。\n";
    let span = ByteSpan::new(text, 3, 12).unwrap();
    let annotations = [SourceAnnotation {
        span,
        data: serde_json::json!({"bbox": [1,2,3,4], "confidence": null}),
    }];
    let mut input = TextInput::new(text);
    input.source = SourceKind::Ocr;
    input.annotations = &annotations;
    let report = engine(EvaluationConfig::default()).evaluate(input).unwrap();
    assert_eq!(report.original_text, text);
    assert_eq!(report.annotations, annotations);
    assert_eq!(report.verdict, Verdict::Undetermined);
    assert_eq!(report.metrics.slm_calls, 0);
    assert_eq!(report.routing.secondary_needed, None);
    assert!(report
        .scores
        .values()
        .iter()
        .all(|score| score.value.is_none()));
    assert!(report
        .coverage
        .iter()
        .all(|row| row.unassessed_spans == vec![ByteSpan::whole(text)]));
    let json = serde_json::to_value(&report).unwrap();
    assert!(json["scores"]["validity"]["value"].is_null());
    assert!(json.get("correction").is_none());
    let decoded: EvaluationReport = serde_json::from_value(json).unwrap();
    decoded.validate().unwrap();
    assert_eq!(decoded, report);
}
#[test]
fn utf8_spans_reject_mid_character_reversed_and_out_of_bounds_ranges() {
    let text = "猫🙂";
    assert_eq!(ByteSpan::new(text, 3, 7).unwrap().end(), 7);
    for (start, end) in [(1, 3), (3, 6), (7, 3), (0, 8)] {
        assert!(ByteSpan::new(text, start, end).is_err());
    }
    assert!(ByteSpan::new(text, 7, 7).is_ok());
}
#[test]
fn invalid_source_annotation_fails_before_backend_execution() {
    struct PanicAnalyzer;
    impl MorphAnalyzer for PanicAnalyzer {
        fn analyze(&self, _: &str) -> Result<MorphAnalysis, EvaluationError> {
            panic!("backend must not run")
        }
    }
    let bad: ByteSpan = serde_json::from_str(r#"{"start":1,"end":3}"#).unwrap();
    let annotations = [SourceAnnotation {
        span: bad,
        data: serde_json::Value::Null,
    }];
    let mut input = TextInput::new("猫");
    input.annotations = &annotations;
    let engine =
        EvaluationEngine::new(Arc::new(PanicAnalyzer), EvaluationConfig::default()).unwrap();
    assert!(matches!(
        engine.evaluate(input),
        Err(EvaluationError::InvalidSpan { .. })
    ));
}
#[test]
fn scores_reject_nonfinite_out_of_range_and_false_calibration() {
    for value in [f32::NAN, f32::INFINITY, -0.1, 1.1] {
        assert!(UnitScore::new(value).is_err());
    }
    assert!(serde_json::from_str::<UnitScore>("1.5").is_err());
    let mut score = DimensionScore::unassessed(ScoreScope::FullText, ScoreStatus::NotEvaluated);
    score.value = Some(UnitScore::new(1.0).unwrap());
    assert!(score.validate().is_err());
    score.status = ScoreStatus::Evaluated;
    score.method = Some(ScoreMethod::Calibrated);
    assert!(score.validate().is_err());
    score.calibration_id = Some("fixture.calibration.v1".into());
    score.validate().unwrap();
}
struct AllAcceptable;
impl PrimaryDetector for AllAcceptable {
    fn detect(
        &self,
        _: &TextInput<'_>,
        _: &MorphAnalysis,
        config: &EvaluationConfig,
    ) -> Result<PrimaryEvaluation, EvaluationError> {
        let score = |scope| DimensionScore {
            value: Some(UnitScore::new(1.0).unwrap()),
            status: ScoreStatus::Evaluated,
            method: Some(ScoreMethod::Heuristic),
            scope,
            calibration_id: None,
            confidence: None,
        };
        Ok(PrimaryEvaluation {
            verdict: Verdict::Acceptable,
            issues: vec![],
            scores: Scores {
                validity: score(ScoreScope::FullText),
                naturalness: score(ScoreScope::FullText),
                semantic_consistency: score(config.semantic_scope),
            },
        })
    }
}
#[test]
fn missing_reference_cannot_be_acceptable_even_if_detector_claims_it() {
    let config = EvaluationConfig {
        semantic_scope: ScoreScope::Reference,
        ..EvaluationConfig::default()
    };
    let engine = engine(config).with_primary_detector(Arc::new(AllAcceptable));
    let mut input = TextInput::new("料金は100円です。");
    input.reference = Some("   ");
    let report = engine.evaluate(input.clone()).unwrap();
    assert_eq!(report.verdict, Verdict::Undetermined);
    assert_eq!(
        report.scores.semantic_consistency.status,
        ScoreStatus::InsufficientContext
    );
    input.reference = Some("料金は100円です。");
    assert_eq!(engine.evaluate(input).unwrap().verdict, Verdict::Acceptable);
}
#[test]
fn batch_keeps_order_and_isolates_invalid_input() {
    let config = EvaluationConfig {
        max_input_bytes: 3,
        ..EvaluationConfig::default()
    };
    let reports = engine(config).evaluate_batch(&[
        TextInput::new("猫"),
        TextInput::new("長すぎる"),
        TextInput::new(""),
    ]);
    assert_eq!(reports.len(), 3);
    assert_eq!(reports[0].as_ref().unwrap().original_text, "猫");
    assert!(matches!(reports[1], Err(EvaluationError::InvalidInput(_))));
    assert_eq!(reports[2].as_ref().unwrap().verdict, Verdict::Undetermined);
}
#[test]
fn malformed_backend_surfaces_are_errors_not_scores() {
    struct BadAnalyzer;
    impl MorphAnalyzer for BadAnalyzer {
        fn analyze(&self, text: &str) -> Result<MorphAnalysis, EvaluationError> {
            let mut analysis = IdentityAnalyzer.analyze(text)?;
            analysis.morphemes[0].surface = "normalized".into();
            Ok(analysis)
        }
    }
    let engine = EvaluationEngine::new(Arc::new(BadAnalyzer), EvaluationConfig::default()).unwrap();
    assert!(matches!(
        engine.evaluate(TextInput::new("原文")),
        Err(EvaluationError::Contract(_))
    ));
}
#[test]
fn deserialized_reports_validate_schema_coverage_and_mandatory_scores() {
    let report = engine(EvaluationConfig::default())
        .evaluate(TextInput::new("自然な文です。"))
        .unwrap();
    let mut bad = report.clone();
    bad.schema_version = "future.v99".into();
    assert!(bad.validate().is_err());
    bad = report.clone();
    bad.verdict = Verdict::Acceptable;
    assert!(bad.validate().is_err());
    bad = report.clone();
    bad.coverage[0]
        .evaluated_spans
        .push(ByteSpan::whole(&bad.original_text));
    assert!(bad.validate().is_err());
    let mut wire = serde_json::to_value(report).unwrap();
    wire["correction"] = serde_json::json!("訂正");
    assert!(serde_json::from_value::<EvaluationReport>(wire).is_err());
}
