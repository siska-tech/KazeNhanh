use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
fn analysis(text: &str) -> MorphAnalysis {
    MorphAnalysis {
        provenance: vec![ArtifactIdentity {
            component: "analyzer".into(),
            id: "scalar.fixture.v1".into(),
            sha256: None,
        }],
        morphemes: text
            .char_indices()
            .map(|(i, c)| Morpheme {
                span: ByteSpan::new(text, i, i + c.len_utf8()).unwrap(),
                surface: c.to_string(),
                dictionary_form: c.to_string(),
                normalized_form: c.to_string(),
                reading: String::new(),
                part_of_speech: vec![],
                is_oov: c == '🙂',
                dictionary_id: 0,
                synonym_group_ids: vec![],
                cumulative_cost: None,
            })
            .collect(),
    }
}
fn asset() -> StatisticsArtifact {
    let docs = vec![
        ("猫は猫".into(), analysis("猫は猫")),
        ("猫🙂".into(), analysis("猫🙂")),
    ];
    StatisticsArtifact::fit(
        StatisticsMetadata {
            id: "fixture.stats.v1".into(),
            domain: "fixture".into(),
            corpus_id: "synthetic-contract-only".into(),
            corpus_sha256: "a".repeat(64),
            license: "CC0-1.0".into(),
            analyzer: analysis("").provenance,
        },
        &docs,
    )
    .unwrap()
}
#[test]
fn counts_unicode_offsets_and_missing_denominators_are_exact() {
    let stats = LightweightStatistics::new(asset()).unwrap();
    let result = stats
        .observe("猫🙂猫", Some("fixture"), &analysis("猫🙂猫"))
        .unwrap();
    assert_eq!(result.character_pairs.unit_count, 2);
    assert_eq!(result.character_pairs.unseen_count, 1);
    assert_eq!(result.character_pairs.unseen_fraction, Some(0.5));
    assert_eq!(
        result.character_pairs.unseen_events[0].span,
        ByteSpan::new("猫🙂猫", 3, 10).unwrap()
    );
    assert_eq!(result.words.unseen_count, 0);
    assert_eq!(result.word_pairs.unseen_count, 1);
    assert_eq!(result.oov_count, 1);
    let empty = stats.observe("", Some("fixture"), &analysis("")).unwrap();
    assert_eq!(empty.words.unseen_fraction, None);
    assert_eq!(empty.character_pairs.unseen_fraction, None);
    let known = stats
        .observe("猫は猫", Some("fixture"), &analysis("猫は猫"))
        .unwrap();
    assert_eq!(known.words.unseen_fraction, Some(0.0));
}
#[test]
fn unknown_domain_and_changed_dictionary_are_unusable_not_clean() {
    let stats = LightweightStatistics::new(asset()).unwrap();
    for domain in [None, Some("other")] {
        assert_eq!(
            stats.observe("猫", domain, &analysis("猫")).unwrap_err(),
            "statistics_domain_missing_or_mismatch"
        );
    }
    let mut changed = analysis("猫");
    changed.provenance[0].id = "other.dictionary".into();
    assert_eq!(
        stats.observe("猫", Some("fixture"), &changed).unwrap_err(),
        "statistics_analyzer_artifacts_mismatch"
    );
}
#[test]
fn invalid_assets_and_training_surfaces_are_rejected() {
    let good = asset();
    let roundtrip: StatisticsArtifact =
        serde_json::from_str(&serde_json::to_string(&good).unwrap()).unwrap();
    assert_eq!(good, roundtrip);
    let mut bad = good.clone();
    bad.word_count += 1;
    assert!(bad.validate().is_err());
    let mut bad = good.clone();
    bad.metadata.license.clear();
    assert!(bad.validate().is_err());
    let mut bad = good.clone();
    bad.metadata.corpus_sha256 = "unverified".into();
    assert!(bad.validate().is_err());
    let mut bad = good.clone();
    bad.schema_version = "kzn.statistics.v2".into();
    assert!(bad.validate().is_err());
    let mut bad = good.clone();
    bad.character_pairs.insert("猫".into(), 1);
    bad.character_pair_count += 1;
    assert!(bad.validate().is_err());
    let mut bad = good.clone();
    bad.words.insert("は".into(), 0);
    bad.word_count -= 1;
    assert!(bad.validate().is_err());
    let mut bad = good.clone();
    bad.words.insert("猫".into(), 1);
    bad.words.insert("は".into(), 3);
    assert!(bad.validate().is_err());
    let mut bad_analysis = analysis("猫");
    bad_analysis.morphemes[0].surface = "犬".into();
    assert!(
        StatisticsArtifact::fit(good.metadata.clone(), &[("猫".into(), bad_analysis)]).is_err()
    );
    assert!(StatisticsArtifact::fit(good.metadata, &[]).is_err());
}
#[test]
fn event_output_is_bounded_without_losing_late_statistics() {
    let stats = LightweightStatistics::new(asset()).unwrap();
    let text = "犬".repeat(100);
    let output = stats
        .observe(&text, Some("fixture"), &analysis(&text))
        .unwrap();
    assert_eq!(output.words.unseen_count, 100);
    assert_eq!(output.words.unseen_events.len(), 64);
    assert_eq!(output.words.omitted_event_count, 36);
    assert_eq!(output.word_pairs.unseen_count, 99);
}
struct CountingAnalyzer(Arc<AtomicUsize>);
impl MorphAnalyzer for CountingAnalyzer {
    fn analyze(&self, text: &str) -> Result<MorphAnalysis, EvaluationError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(analysis(text))
    }
}
#[test]
fn recognition_shares_one_analysis_and_never_promotes_frequency_to_risk() {
    let calls = Arc::new(AtomicUsize::new(0));
    let engine = RecognitionEngine::new(
        Arc::new(CountingAnalyzer(calls.clone())),
        RecognitionConfig::default(),
    )
    .unwrap()
    .with_statistics(Arc::new(LightweightStatistics::new(asset()).unwrap()));
    for source in [RecognitionSource::Ocr, RecognitionSource::Asr] {
        for text in ["猫は猫", "犬🙂", "体系キープ"] {
            let mut input = RecognitionInput::new(text, source, "d", "s");
            input.domain = Some("fixture");
            let report = engine.evaluate_recognition(input).unwrap();
            report.validate().unwrap();
            assert_eq!(report.decision, RecognitionDecision::Undetermined);
            assert_eq!(report.recognition_risk.value, None);
            assert_eq!(report.naturalness.value, None);
            assert_eq!(report.metrics.slm_calls, 0);
            assert_eq!(
                report
                    .evidence
                    .iter()
                    .find(|e| e.kind == RecognitionEvidenceKind::LexicalStatistics)
                    .unwrap()
                    .status,
                RecognitionEvidenceStatus::Observed
            );
        }
    }
    assert_eq!(calls.load(Ordering::SeqCst), 6);
    let report = engine
        .evaluate_recognition(RecognitionInput::new(
            "猫",
            RecognitionSource::Ocr,
            "d",
            "s",
        ))
        .unwrap();
    assert!(report
        .reasons
        .contains(&"statistics_domain_missing_or_mismatch".into()));
    let annotations = [SourceAnnotation {
        span: ByteSpan::new("abc", 1, 2).unwrap(),
        data: serde_json::Value::Null,
    }];
    let mut bad = RecognitionInput::new("猫", RecognitionSource::Ocr, "d", "s");
    bad.annotations = &annotations;
    assert!(engine.evaluate_recognition(bad).is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 7);
}

#[test]
fn report_rejects_forged_statistics_counts_spans_and_provider() {
    let engine = RecognitionEngine::new(
        Arc::new(CountingAnalyzer(Arc::new(AtomicUsize::new(0)))),
        RecognitionConfig::default(),
    )
    .unwrap()
    .with_statistics(Arc::new(LightweightStatistics::new(asset()).unwrap()));
    let mut input = RecognitionInput::new("猫🙂犬", RecognitionSource::Asr, "d", "s");
    input.domain = Some("fixture");
    let report = engine.evaluate_recognition(input).unwrap();
    let roundtrip: RecognitionReport =
        serde_json::from_str(&serde_json::to_string(&report).unwrap()).unwrap();
    roundtrip.validate().unwrap();
    let index = report
        .evidence
        .iter()
        .position(|e| e.kind == RecognitionEvidenceKind::LexicalStatistics)
        .unwrap();
    let mut bad = report.clone();
    bad.evidence[index].value.as_mut().unwrap()["words"]["unseen_fraction"] =
        serde_json::json!(0.0);
    assert!(bad.validate().is_err());
    let mut bad = report.clone();
    bad.evidence[index].provider_id = "wrong".into();
    assert!(bad.validate().is_err());
    let mut bad = report.clone();
    bad.evidence[index].value.as_mut().unwrap()["word_pairs"]["unit_count"] = serde_json::json!(5);
    assert!(bad.validate().is_err());
    let mut bad = report;
    bad.evidence[index].value.as_mut().unwrap()["character_pairs"]["unseen_events"][0]["span"] =
        serde_json::json!({"start":4,"end":7});
    assert!(bad.validate().is_err());
}

fn sparse_engine() -> RecognitionEngine {
    RecognitionEngine::new(
        Arc::new(CountingAnalyzer(Arc::new(AtomicUsize::new(0)))),
        RecognitionConfig::default(),
    )
    .unwrap()
    .with_statistics(Arc::new(LightweightStatistics::new(asset()).unwrap()))
    .with_sparse_statistics_review()
}
#[test]
fn sparse_review_requires_all_three_observations_and_applicable_asset() {
    let engine = sparse_engine();
    for source in [RecognitionSource::Ocr, RecognitionSource::Asr] {
        for (text, expected) in [
            ("猫🙂犬", RecognitionDecision::Review),
            ("猫犬", RecognitionDecision::Undetermined),
            ("猫🙂", RecognitionDecision::Undetermined),
            ("🙂", RecognitionDecision::Undetermined),
            ("", RecognitionDecision::Undetermined),
        ] {
            let mut input = RecognitionInput::new(text, source, "doc", "s");
            input.domain = Some("fixture");
            let report = engine.evaluate_recognition(input).unwrap();
            assert_eq!(report.decision, expected, "{text}");
            assert!(report.recognition_risk.value.is_none());
            assert_eq!(report.metrics.slm_calls, 0);
        }
    }
    let mut input = RecognitionInput::new("猫🙂犬", RecognitionSource::Asr, "doc", "s");
    input.domain = Some("unknown");
    assert_eq!(
        engine.evaluate_recognition(input).unwrap().decision,
        RecognitionDecision::Undetermined
    );
    let engine = RecognitionEngine::new(
        Arc::new(CountingAnalyzer(Arc::new(AtomicUsize::new(0)))),
        RecognitionConfig::default(),
    )
    .unwrap()
    .with_sparse_statistics_review();
    assert_eq!(
        engine
            .evaluate_recognition(RecognitionInput::new(
                "猫🙂犬",
                RecognitionSource::Ocr,
                "d",
                "s"
            ))
            .unwrap()
            .decision,
        RecognitionDecision::Undetermined
    );
    assert_eq!(
        engine
            .evaluate_recognition(RecognitionInput::new(
                "猫猫猫猫猫猫",
                RecognitionSource::Ocr,
                "d",
                "s"
            ))
            .unwrap()
            .decision,
        RecognitionDecision::Review
    );
}
#[test]
fn sparse_report_rejects_forged_findings_reasons_and_policy() {
    let engine = sparse_engine();
    let mut input = RecognitionInput::new("猫🙂犬", RecognitionSource::Ocr, "doc", "s");
    input.domain = Some("fixture");
    let report = engine.evaluate_recognition(input).unwrap();
    let roundtrip: RecognitionReport =
        serde_json::from_str(&serde_json::to_string(&report).unwrap()).unwrap();
    roundtrip.validate().unwrap();
    let index = report
        .findings
        .iter()
        .position(|f| f.issue.code == "sparse_statistics_conjunction")
        .unwrap();
    let mut bad = report.clone();
    bad.findings.remove(index);
    assert!(bad.validate().is_err());
    let mut bad = report.clone();
    bad.findings[index].issue.evidence["oov_count"] = serde_json::json!(999);
    assert!(bad.validate().is_err());
    let mut bad = report.clone();
    bad.findings[index].issue.span = ByteSpan::new("猫🙂犬", 0, 3).unwrap();
    assert!(bad.validate().is_err());
    let mut bad = report.clone();
    bad.reasons
        .retain(|r| r != "sparse_statistics_require_review");
    assert!(bad.validate().is_err());
    let mut bad = report.clone();
    bad.decision_policy.id = "kzn.recognition.abstain.v1".into();
    assert!(bad.validate().is_err());
    let mut bad = report;
    bad.decision = RecognitionDecision::Undetermined;
    assert!(bad.validate().is_err());
}
#[test]
fn combined_candidate_and_statistics_review_preserves_high_confidence_without_override() {
    let text = "猫🙂犬";
    let mut candidates = CandidateEvidence {
        status: RecognitionEvidenceStatus::Observed,
        hypotheses: vec![text, "猫🙂鳥"]
            .into_iter()
            .enumerate()
            .map(|(i, text)| RecognitionCandidate {
                rank: i + 1,
                text: text.into(),
                scores: vec![],
                alignment: vec![],
            })
            .collect(),
        truncated: Some(true),
        origin: Some("synthetic".into()),
        reason: None,
    };
    candidates.align(text).unwrap();
    let evidence = RecognizerEvidence {
        schema_version: RECOGNIZER_EVIDENCE_SCHEMA.into(),
        source: RecognitionSource::Asr,
        recognizer: RecognizerIdentity {
            engine: "synthetic".into(),
            model: None,
            version: None,
            decoder: None,
        },
        profile: RecognitionSourceProfile::new("synthetic", RecognitionSource::Asr),
        candidates,
        anchors: vec![],
        confidences: vec![ConfidenceObservation {
            id: "high".into(),
            span: ByteSpan::whole(text),
            granularity: ConfidenceGranularity::Segment,
            status: RecognitionEvidenceStatus::Observed,
            score: Some(RawScore {
                value: 0.999,
                meaning: ConfidenceMeaning::EngineScore,
                direction: ConfidenceDirection::Unknown,
                range: None,
                calibration_id: None,
                target: None,
            }),
            aggregation: None,
            reason: None,
            dependencies: vec![],
        }],
    };
    let mut input = RecognitionInput::new(text, RecognitionSource::Asr, "d", "s");
    input.domain = Some("fixture");
    input.recognizer_evidence = Some(&evidence);
    let report = sparse_engine()
        .with_candidate_disagreement_review()
        .evaluate_recognition(input)
        .unwrap();
    assert_eq!(report.decision_policy.id, SPARSE_CANDIDATE_REVIEW_POLICY_ID);
    assert_eq!(report.recognizer_evidence.as_ref(), Some(&evidence));
    assert!(report
        .findings
        .iter()
        .any(|f| f.issue.code == "candidate_disagreement"));
    assert!(report
        .findings
        .iter()
        .any(|f| f.issue.code == "sparse_statistics_conjunction"));
    assert_eq!(report.decision, RecognitionDecision::Review);
    report.validate().unwrap();
}
