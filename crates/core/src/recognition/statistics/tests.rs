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

fn pos_analysis(text: &str, tags: &[&str]) -> MorphAnalysis {
    let mut result = analysis(text);
    assert_eq!(result.morphemes.len(), tags.len());
    for (token, tag) in result.morphemes.iter_mut().zip(tags) {
        token.part_of_speech = if tag.is_empty() {
            vec![]
        } else {
            vec![(*tag).into(), "*".into()]
        };
    }
    result
}
fn pos_asset() -> StatisticsArtifact {
    StatisticsArtifact::fit_with_pos(
        asset().metadata,
        &[(
            "猫は犬".into(),
            pos_analysis("猫は犬", &["名詞", "助詞", "名詞"]),
        )],
    )
    .unwrap()
}
#[test]
fn pos_pairs_use_full_vectors_and_do_not_bridge_missing_tags() {
    let provider = LightweightStatistics::new(pos_asset()).unwrap();
    let known = provider
        .observe(
            "猫は犬",
            Some("fixture"),
            &pos_analysis("猫は犬", &["名詞", "助詞", "名詞"]),
        )
        .unwrap();
    assert_eq!(
        known
            .pos
            .as_ref()
            .unwrap()
            .frequencies
            .as_ref()
            .unwrap()
            .unseen_fraction,
        Some(0.0)
    );
    let changed = provider
        .observe(
            "猫は犬",
            Some("fixture"),
            &pos_analysis("猫は犬", &["名詞", "動詞", "名詞"]),
        )
        .unwrap();
    let pos = changed.pos.as_ref().unwrap();
    assert_eq!(pos.available_pair_count, 2);
    assert_eq!(pos.frequencies.as_ref().unwrap().unseen_count, 2);
    assert_eq!(
        pos.frequencies.as_ref().unwrap().unseen_events[0].span,
        ByteSpan::new("猫は犬", 0, 6).unwrap()
    );
    changed
        .validate("猫は犬", Some("fixture"), provider.artifact_id())
        .unwrap();
    let mut detailed = pos_analysis("猫は犬", &["名詞", "助詞", "名詞"]);
    detailed.morphemes[1].part_of_speech[1] = "格助詞".into();
    assert_eq!(
        provider
            .observe("猫は犬", Some("fixture"), &detailed)
            .unwrap()
            .pos
            .unwrap()
            .frequencies
            .unwrap()
            .unseen_count,
        2
    );
    for missing in ["", "*"] {
        let result = provider
            .observe(
                "猫は犬",
                Some("fixture"),
                &pos_analysis("猫は犬", &["名詞", missing, "名詞"]),
            )
            .unwrap();
        let pos = result.pos.as_ref().unwrap();
        assert_eq!(pos.missing_pair_count, 2);
        assert_eq!(pos.available_pair_count, 0);
        assert_eq!(pos.frequencies.as_ref().unwrap().unseen_fraction, None);
        result
            .validate("猫は犬", Some("fixture"), provider.artifact_id())
            .unwrap();
    }
}
#[test]
fn legacy_assets_and_empty_pos_training_remain_distinct() {
    let legacy = asset();
    let json = serde_json::to_string(&legacy).unwrap();
    assert!(!json.contains("\"pos\""));
    let legacy: StatisticsArtifact = serde_json::from_str(&json).unwrap();
    legacy.validate().unwrap();
    let legacy_provider = LightweightStatistics::new(legacy).unwrap();
    assert!(legacy_provider
        .observe("猫", Some("fixture"), &analysis("猫"))
        .unwrap()
        .pos
        .is_none());
    let empty =
        StatisticsArtifact::fit_with_pos(asset().metadata, &[("猫犬".into(), analysis("猫犬"))])
            .unwrap();
    let provider = LightweightStatistics::new(empty).unwrap();
    let result = provider
        .observe(
            "猫犬",
            Some("fixture"),
            &pos_analysis("猫犬", &["名詞", "名詞"]),
        )
        .unwrap();
    let pos = result.pos.as_ref().unwrap();
    assert_eq!(pos.corpus_pair_count, 0);
    assert_eq!(pos.available_pair_count, 1);
    assert!(pos.frequencies.is_none());
    result
        .validate("猫犬", Some("fixture"), provider.artifact_id())
        .unwrap();
}
#[test]
fn pos_asset_and_observation_tampering_are_rejected() {
    let good = pos_asset();
    let mut bad = good.clone();
    bad.pos = None;
    assert!(bad.validate().is_err());
    let mut bad = good.clone();
    bad.schema_version = STATISTICS_SCHEMA.into();
    assert!(bad.validate().is_err());
    let mut bad = good.clone();
    bad.pos.as_mut().unwrap().pair_count += 1;
    assert!(bad.validate().is_err());
    let mut bad = good.clone();
    bad.pos.as_mut().unwrap().missing_pair_count = u64::MAX;
    assert!(bad.validate().is_err());
    let mut bad = good.clone();
    let pos = bad.pos.as_mut().unwrap();
    pos.pairs = BTreeMap::from([("[[\"名詞\"],[]]".into(), 2)]);
    assert!(bad.validate().is_err());
    let provider = LightweightStatistics::new(good).unwrap();
    let report = provider
        .observe(
            "猫は犬",
            Some("fixture"),
            &pos_analysis("猫は犬", &["動詞", "動詞", "動詞"]),
        )
        .unwrap();
    let mut bad = report.clone();
    bad.pos.as_mut().unwrap().missing_pair_count = usize::MAX;
    assert!(bad
        .validate("猫は犬", Some("fixture"), provider.artifact_id())
        .is_err());
    let mut bad = report.clone();
    bad.pos.as_mut().unwrap().frequencies = None;
    assert!(bad
        .validate("猫は犬", Some("fixture"), provider.artifact_id())
        .is_err());
    let mut bad = report;
    bad.pos
        .as_mut()
        .unwrap()
        .frequencies
        .as_mut()
        .unwrap()
        .unseen_events[0]
        .span = ByteSpan::new("猫は犬", 0, 0).unwrap();
    assert!(bad
        .validate("猫は犬", Some("fixture"), provider.artifact_id())
        .is_err());
}
#[test]
fn pos_event_limits_keep_counts_without_cross_document_pairs() {
    let data = vec![
        ("猫".into(), pos_analysis("猫", &["名詞"])),
        ("犬".into(), pos_analysis("犬", &["名詞"])),
    ];
    let separate = StatisticsArtifact::fit_with_pos(asset().metadata, &data).unwrap();
    assert_eq!(separate.pos.unwrap().pair_count, 0);
    let provider = LightweightStatistics::new(pos_asset()).unwrap();
    let text = "猫".repeat(100);
    let result = provider
        .observe(
            &text,
            Some("fixture"),
            &pos_analysis(&text, &vec!["動詞"; 100]),
        )
        .unwrap();
    let f = result.pos.as_ref().unwrap().frequencies.as_ref().unwrap();
    assert_eq!(f.unseen_count, 99);
    assert_eq!(f.unseen_events.len(), 64);
    assert_eq!(f.omitted_event_count, 35);
    result
        .validate(&text, Some("fixture"), provider.artifact_id())
        .unwrap();
}

#[test]
fn pos_engine_roundtrip_shares_analysis_without_changing_decision() {
    struct Tagged(Arc<AtomicUsize>);
    impl MorphAnalyzer for Tagged {
        fn analyze(&self, text: &str) -> Result<MorphAnalysis, EvaluationError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(pos_analysis(text, &vec!["動詞"; text.chars().count()]))
        }
    }
    let calls = Arc::new(AtomicUsize::new(0));
    let engine = RecognitionEngine::new(
        Arc::new(Tagged(calls.clone())),
        RecognitionConfig::default(),
    )
    .unwrap()
    .with_statistics(Arc::new(LightweightStatistics::new(pos_asset()).unwrap()));
    let mut input = RecognitionInput::new("猫は犬", RecognitionSource::Asr, "d", "s");
    input.domain = Some("fixture");
    let report = engine.evaluate_recognition(input).unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(report.decision, RecognitionDecision::Undetermined);
    assert!(report.recognition_risk.value.is_none());
    assert_eq!(report.metrics.slm_calls, 0);
    let roundtrip: RecognitionReport =
        serde_json::from_str(&serde_json::to_string(&report).unwrap()).unwrap();
    roundtrip.validate().unwrap();
    let mut bad = report;
    let row = bad
        .evidence
        .iter_mut()
        .find(|e| e.kind == RecognitionEvidenceKind::LexicalStatistics)
        .unwrap();
    row.value.as_mut().unwrap()["pos"]["frequencies"]["unseen_fraction"] = serde_json::json!(0.0);
    assert!(bad.validate().is_err());
}
