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
