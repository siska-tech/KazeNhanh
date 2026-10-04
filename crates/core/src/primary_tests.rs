use super::*;
struct WholeToken;
impl MorphAnalyzer for WholeToken {
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
                    part_of_speech: vec!["名詞".into()],
                    is_oov: true,
                    dictionary_id: -1,
                    synonym_group_ids: vec![],
                    cumulative_cost: Some(-10000),
                }]
            },
            provenance: vec![],
        })
    }
}
fn evaluate(text: &str, profile: DomainProfile, reference: Option<&str>) -> EvaluationReport {
    let config = profile.evaluation_config();
    let engine = EvaluationEngine::new(Arc::new(WholeToken), config)
        .unwrap()
        .with_primary_detector(Arc::new(PrimaryRules::new(profile).unwrap()));
    let mut input = TextInput::new(text);
    input.reference = reference;
    engine.evaluate(input).unwrap()
}
#[test]
fn unknown_proper_names_products_and_costs_are_not_error_probabilities() {
    for text in [
        "KazeNhanh",
        "新製品XYZ-000000",
        "髙橋",
        "🙂🙂🙂🙂🙂🙂",
        "cafe\u{301}",
        "2026年10月",
    ] {
        let report = evaluate(text, DomainProfile::screening(), None);
        assert_eq!(report.verdict, Verdict::Acceptable, "{text}");
        assert!(report.issues.is_empty());
        assert_eq!(report.metrics.morphology.oov_count, 1);
        assert!(report.scores.validity.confidence.is_none());
        assert!(report.scores.semantic_consistency.value.is_none());
        assert_eq!(report.routing.secondary_needed, Some(false));
    }
}
#[test]
fn warnings_preserve_raw_spans_and_never_produce_correction_or_semantic_pass() {
    let text = "🙂ああああああ（欠落�";
    let report = evaluate(text, DomainProfile::screening(), None);
    assert_eq!(report.verdict, Verdict::Suspicious);
    assert_eq!(report.routing.status, RoutingStatus::Disabled);
    for code in [
        "repeated_character",
        "unmatched_bracket",
        "replacement_character",
    ] {
        assert!(report.issues.iter().any(|i| i.code == code));
    }
    let repeated = report
        .issues
        .iter()
        .find(|i| i.code == "repeated_character")
        .unwrap();
    assert_eq!(
        &text[repeated.span.start()..repeated.span.end()],
        "ああああああ"
    );
    assert_eq!(report.metrics.slm_calls, 0);
    report.validate().unwrap();
    let json = serde_json::to_value(report).unwrap();
    assert!(json.get("correction").is_none());
}
#[test]
fn form_constraints_and_optional_empty_are_distinct_from_api_errors() {
    let mut profile = DomainProfile::builtin("ja.form.v1").unwrap();
    profile.id = "form.numeric.v1".into();
    profile.max_chars = Some(3);
    profile.format = TextFormat::AsciiDigits;
    assert_eq!(
        evaluate("123", profile.clone(), None).verdict,
        Verdict::Acceptable
    );
    for text in ["", "1234", "１２３", "12x"] {
        let report = evaluate(text, profile.clone(), None);
        assert_eq!(report.verdict, Verdict::Invalid);
        assert_eq!(report.routing.secondary_needed, Some(false));
    }
    let mut unicode_profile = profile.clone();
    unicode_profile.format = TextFormat::FreeText;
    unicode_profile.max_chars = Some(2);
    assert_eq!(
        evaluate("東京", unicode_profile.clone(), None).verdict,
        Verdict::Acceptable
    );
    assert_eq!(
        evaluate("東京都", unicode_profile, None).verdict,
        Verdict::Invalid
    );
    profile.required = false;
    let report = evaluate("  ", profile, None);
    assert_eq!(report.verdict, Verdict::Acceptable);
    assert_eq!(report.scores.naturalness.status, ScoreStatus::NotApplicable);
}
#[test]
fn reference_requests_and_missing_context_remain_unresolved() {
    let profile = DomainProfile::builtin("ja.llm.v1").unwrap();
    let missing = evaluate("料金は100円です。", profile.clone(), None);
    assert_eq!(missing.verdict, Verdict::Undetermined);
    assert_eq!(
        missing.scores.semantic_consistency.status,
        ScoreStatus::InsufficientContext
    );
    let conflict = evaluate("料金は100円です。", profile, Some("料金は200円です。"));
    assert_eq!(conflict.verdict, Verdict::Undetermined);
    assert_eq!(conflict.routing.secondary_needed, Some(true));
    assert!(conflict.scores.semantic_consistency.value.is_none());
}
#[test]
fn allowlist_is_explicit_and_profile_snapshot_is_serialized() {
    let mut profile = DomainProfile::screening();
    profile.id = "chat.expressive.v1".into();
    profile.allowed_terms.push("ああああああ".into());
    let report = evaluate("ああああああ", profile.clone(), None);
    assert_eq!(report.verdict, Verdict::Acceptable);
    assert_eq!(report.profile_config, Some(profile));
    assert!(report.provenance.iter().any(|p| p.id == PRIMARY_RULES_ID));
}
#[test]
fn output_is_bounded_but_late_constraint_violations_are_not_hidden() {
    let text = format!("{}\u{0}", "�".repeat(1024));
    let report = evaluate(&text, DomainProfile::screening(), None);
    assert_eq!(report.verdict, Verdict::Invalid);
    assert_eq!(report.issues.len(), 257);
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.code == "forbidden_control"));
    assert!(report
        .issues
        .iter()
        .any(|i| i.code == "findings_truncated" && i.evidence["omitted"].as_u64().unwrap() > 0));
}
#[test]
fn profile_errors_do_not_silently_fall_back_to_screening() {
    assert!(DomainProfile::builtin("not-installed").is_err());
    let mut profile = DomainProfile::screening();
    profile.repeated_token_min = Some(1);
    assert!(PrimaryRules::new(profile).is_err());
}

#[test]
fn report_validation_rejects_false_pass_and_disabled_inference_claims() {
    let mut report = evaluate("欠落�", DomainProfile::screening(), None);
    report.verdict = Verdict::Acceptable;
    assert!(report.validate().is_err());
    report.verdict = Verdict::Suspicious;
    report.metrics.slm_calls = 1;
    assert!(report.validate().is_err());
}
