use kaze_nhanh::{
    japanese_engine, EvaluationConfig, EvaluationEngine, EvaluationError, MorphAnalyzer,
    SourceKind, SudachiAnalyzer, SudachiConfig, SudachiMode, TextInput, Verdict,
};
use std::{
    path::PathBuf,
    sync::{Arc, OnceLock},
};
fn assets(mode: SudachiMode) -> SudachiConfig {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    SudachiConfig::from_paths(
        root.join("resources/sudachi/system.dic"),
        root.join("resources/sudachi/sudachi.json"),
        mode,
    )
    .unwrap()
}
fn analyzer() -> Arc<SudachiAnalyzer> {
    static ANALYZER: OnceLock<Arc<SudachiAnalyzer>> = OnceLock::new();
    Arc::clone(
        ANALYZER.get_or_init(|| Arc::new(SudachiAnalyzer::new(assets(SudachiMode::C)).unwrap())),
    )
}
#[test]
fn owned_japanese_engine_starts_without_any_model_and_preserves_all_four_sources() {
    let engine = japanese_engine(assets(SudachiMode::C), EvaluationConfig::default()).unwrap();
    let text = " \r\n東京都で自然な日本語🙂を解析します。 cafe\u{301} ＡＢＣ\n";
    for source in [
        SourceKind::Ocr,
        SourceKind::Asr,
        SourceKind::Llm,
        SourceKind::Form,
    ] {
        let mut input = TextInput::new(text);
        input.source = source.clone();
        let report = engine.evaluate(input).unwrap();
        assert_eq!(report.original_text, text);
        assert_eq!(report.source, source);
        assert_eq!(report.verdict, Verdict::Acceptable);
        assert_eq!(report.metrics.slm_calls, 0);
        assert!(report.metrics.morpheme_count > 3);
        assert!(report.scores.naturalness.value.is_some());
        assert!(report.scores.semantic_consistency.value.is_none());
        assert!(report
            .provenance
            .iter()
            .any(|item| item.component == "dictionary"
                && item.sha256.as_deref()
                    == Some("edd7e6521ccc2c2e674dd5f4cccb9e87456cf12507b8f8448973f4602b545317")));
        report.validate().unwrap();
    }
}
#[test]
fn adapter_preserves_all_pos_surface_order_and_backend_neutral_spans() {
    let text = "東京都で自然な日本語を解析します。 未登録XYZ123🌀";
    let analysis = analyzer().analyze(text).unwrap();
    assert!(analysis
        .morphemes
        .iter()
        .any(|m| m.part_of_speech.first().map(String::as_str) == Some("助詞")));
    assert!(analysis.morphemes.iter().any(|m| m.is_oov));
    let mut end = 0;
    for morpheme in analysis.morphemes {
        assert!(morpheme.span.start() >= end);
        assert_eq!(
            &text[morpheme.span.start()..morpheme.span.end()],
            morpheme.surface
        );
        assert!(!morpheme.part_of_speech.is_empty());
        assert!(morpheme.cumulative_cost.is_some());
        end = morpheme.span.end();
    }
    assert_eq!(end, text.len());
}
#[test]
fn mode_and_asset_identity_are_explicit() {
    for (mode, label) in [
        (SudachiMode::A, "A"),
        (SudachiMode::B, "B"),
        (SudachiMode::C, "C"),
    ] {
        let analysis = SudachiAnalyzer::new(assets(mode))
            .unwrap()
            .analyze("関西国際空港へ行く。")
            .unwrap();
        assert!(!analysis.morphemes.is_empty());
        assert!(analysis
            .provenance
            .iter()
            .any(|id| id.id == format!("sudachi.rs.v0.6.9.mode.{label}")));
        assert!(
            analysis
                .provenance
                .iter()
                .filter(|id| id.sha256.is_some())
                .count()
                == 2
        );
    }
}
#[test]
fn invalid_resources_are_backend_errors_not_reports() {
    let config = SudachiConfig {
        dictionary: b"not-a-dictionary".to_vec(),
        settings: b"{}".to_vec(),
        mode: SudachiMode::C,
    };
    assert!(matches!(
        SudachiAnalyzer::new(config),
        Err(EvaluationError::Backend { .. })
    ));
}
#[test]
fn shared_dictionary_supports_concurrent_evaluation() {
    let engine = Arc::new(EvaluationEngine::new(analyzer(), EvaluationConfig::default()).unwrap());
    let workers = (0..4)
        .map(|_| {
            let engine = Arc::clone(&engine);
            std::thread::spawn(move || engine.evaluate(TextInput::new("短い入力です。")))
        })
        .collect::<Vec<_>>();
    for worker in workers {
        let report = worker.join().unwrap().unwrap();
        assert_eq!(report.verdict, Verdict::Undetermined);
    }
}

#[test]
fn real_primary_rules_keep_auxiliaries_and_return_the_original_repeated_range() {
    let rules = kaze_nhanh::PrimaryRules::new(kaze_nhanh::DomainProfile::screening()).unwrap();
    let engine = EvaluationEngine::new(analyzer(), EvaluationConfig::default())
        .unwrap()
        .with_primary_detector(Arc::new(rules));
    let text = "今日は晴れですですです。";
    let report = engine.evaluate(TextInput::new(text)).unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.code == "repeated_token")
        .unwrap();
    assert_eq!(&text[issue.span.start()..issue.span.end()], "ですですです");
    assert!(report.metrics.morphology.auxiliary_count >= 3);
    assert!(report.metrics.morphology.particle_count >= 1);
    assert_eq!(report.verdict, Verdict::Suspicious);
    assert_eq!(report.metrics.slm_calls, 0);
}

#[test]
fn real_sudachi_routes_asr_auxiliary_repetition_to_explicit_fake_only() {
    use kaze_nhanh::*;
    use std::{
        sync::atomic::{AtomicUsize, Ordering},
        time::Instant,
    };
    struct Factory(Arc<AtomicUsize>);
    impl SecondaryJudgeFactory for Factory {
        fn artifacts(&self) -> Vec<ArtifactIdentity> {
            vec![ArtifactIdentity {
                component: "fixture".into(),
                id: "asr.explicit-fake.v1".into(),
                sha256: None,
            }]
        }
        fn load(&self, _: Instant) -> Result<Box<dyn SecondaryJudge>, SecondaryFailure> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(Box::new(Judge))
        }
    }
    struct Judge;
    impl SecondaryJudge for Judge {
        fn judge(
            &mut self,
            request: &SecondaryRequest,
        ) -> Result<SecondaryEvaluation, SecondaryFailure> {
            assert_eq!(request.text, "今日は晴れですですです。");
            assert_eq!(request.profile_id, "ja.asr.v1");
            let issue = request
                .primary_issues
                .iter()
                .find(|i| i.code == "repeated_token")
                .unwrap();
            assert_eq!(
                &request.text[issue.span.start()..issue.span.end()],
                "ですですです"
            );
            Ok(SecondaryEvaluation {
                naturalness: Some(UnitScore::new(0.2).unwrap()),
                semantic_consistency: None,
                issues: vec![],
            })
        }
    }
    let loads = Arc::new(AtomicUsize::new(0));
    let worker = Arc::new(
        SecondaryWorker::new(Arc::new(Factory(loads.clone())), SecondaryPolicy::default()).unwrap(),
    );
    let profile = DomainProfile::builtin("ja.asr.v1").unwrap();
    let engine = EvaluationEngine::new(analyzer(), profile.evaluation_config())
        .unwrap()
        .with_primary_detector(Arc::new(PrimaryRules::new(profile).unwrap()))
        .with_secondary_worker(worker);
    let normal = engine.evaluate(TextInput::new("今日は晴れです。")).unwrap();
    assert_eq!(normal.metrics.slm_calls, 0);
    assert_eq!(loads.load(Ordering::SeqCst), 0);
    let mut input = TextInput::new("今日は晴れですですです。");
    input.source = SourceKind::Asr;
    let report = engine.evaluate(input).unwrap();
    assert_eq!(report.source, SourceKind::Asr);
    assert_eq!(report.metrics.slm_calls, 1);
    assert_eq!(report.routing.status, RoutingStatus::Completed);
    assert_eq!(report.verdict, Verdict::Suspicious);
    assert_eq!(loads.load(Ordering::SeqCst), 1);
    assert!(report.metrics.morphology.auxiliary_count >= 3);
    report.validate().unwrap();
}

#[test]
fn recognition_r0_preserves_fifteen_ocr_samples_and_abstains_without_risk_evidence() {
    use kaze_nhanh::*;
    let engine = RecognitionEngine::new(analyzer(), RecognitionConfig::default()).unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut count = 0;
    for suffix in ["001", "002", "003"] {
        let data = std::fs::read_to_string(
            root.join(format!("evaluation/ppocrv6-medium-user-{suffix}.jsonl")),
        )
        .unwrap();
        for line in data.lines().filter(|line| !line.trim().is_empty()) {
            let sample: serde_json::Value = serde_json::from_str(line).unwrap();
            let text = sample["text"].as_str().unwrap();
            let doc = format!("user-image-{suffix}");
            let annotations = [SourceAnnotation {
                span: ByteSpan::whole(text),
                data: serde_json::json!({"ocr_confidence":sample["confidence"],"ocr_engine":sample["engine"]}),
            }];
            let mut input = RecognitionInput::new(
                text,
                RecognitionSource::Ocr,
                &doc,
                sample["id"].as_str().unwrap(),
            );
            input.annotations = &annotations;
            let result = engine.evaluate_recognition(input).unwrap();
            assert_eq!(result.original_text, text);
            assert_eq!(result.annotations, annotations);
            assert_eq!(result.decision, RecognitionDecision::Undetermined);
            assert_eq!(result.recognition_risk.value, None);
            assert_eq!(result.metrics.slm_calls, 0);
            assert!(result
                .provenance
                .iter()
                .any(|a| a.component == "dictionary"));
            result.validate().unwrap();
            count += 1;
        }
    }
    assert_eq!(count, 15);
    let asr = engine
        .evaluate_recognition(RecognitionInput::new(
            "今日は晴れです",
            RecognitionSource::Asr,
            "utterance",
            "1",
        ))
        .unwrap();
    assert_eq!(asr.decision, RecognitionDecision::Undetermined);
    let anomaly = engine
        .evaluate_recognition(RecognitionInput::new(
            "猫猫猫猫猫猫",
            RecognitionSource::Asr,
            "utterance",
            "2",
        ))
        .unwrap();
    assert_eq!(anomaly.decision, RecognitionDecision::Review);
}

#[test]
fn real_sudachi_statistics_share_artifacts_and_preserve_hard_clean_abstentions() {
    use kaze_nhanh::{
        LightweightStatistics, RecognitionConfig, RecognitionDecision, RecognitionEngine,
        RecognitionEvidenceKind, RecognitionEvidenceStatus, RecognitionInput, RecognitionSource,
        StatisticsArtifact, StatisticsMetadata,
    };
    use sha2::{Digest, Sha256};
    let bytes = include_bytes!("fixtures/statistics/clean-contract.jsonl");
    let documents: Vec<_> = std::str::from_utf8(bytes)
        .unwrap()
        .lines()
        .map(|line| {
            let sample: serde_json::Value = serde_json::from_str(line).unwrap();
            let text = sample["text"].as_str().unwrap().to_owned();
            let analysis = analyzer().analyze(&text).unwrap();
            (text, analysis)
        })
        .collect();
    let asset = StatisticsArtifact::fit(
        StatisticsMetadata {
            id: "contract.stats.v1".into(),
            domain: "contract_fixture".into(),
            corpus_id: "authored-contract-only".into(),
            corpus_sha256: format!("{:x}", Sha256::digest(bytes)),
            license: "CC0-1.0".into(),
            analyzer: documents[0].1.provenance.clone(),
        },
        &documents,
    )
    .unwrap();
    let provider = Arc::new(LightweightStatistics::new(asset).unwrap());
    let engine = RecognitionEngine::new(analyzer(), RecognitionConfig::default())
        .unwrap()
        .with_statistics(provider.clone());
    // These are authored contract cases, not real ASR measurements or quality acceptance.
    for source in [RecognitionSource::Ocr, RecognitionSource::Asr] {
        for text in [
            "体系キープ",
            "あの、その、はい。",
            "型番ZX-900B",
            "青凪さん、ええと、明日で。",
            "猫猫猫猫猫猫",
        ] {
            let mut input = RecognitionInput::new(text, source, "contract-only", "held-clean");
            input.domain = Some("contract_fixture");
            let report = engine.evaluate_recognition(input).unwrap();
            assert_eq!(report.original_text, text);
            assert_eq!(report.recognition_risk.value, None);
            assert_eq!(report.metrics.slm_calls, 0);
            assert_ne!(report.decision, RecognitionDecision::LowRisk);
            let row = report
                .evidence
                .iter()
                .find(|e| e.kind == RecognitionEvidenceKind::LexicalStatistics)
                .unwrap();
            assert_eq!(row.status, RecognitionEvidenceStatus::Observed);
            if text == "猫猫猫猫猫猫" {
                assert_eq!(report.decision, RecognitionDecision::Review);
            } else {
                assert_eq!(report.decision, RecognitionDecision::Undetermined);
            }
        }
    }
    // Explicit counterexample: a legitimate product code can trigger experimental review.
    // Do not interpret this policy as an accepted correctness classifier.
    let experimental = RecognitionEngine::new(analyzer(), RecognitionConfig::default())
        .unwrap()
        .with_statistics(provider.clone())
        .with_sparse_statistics_review();
    for source in [RecognitionSource::Ocr, RecognitionSource::Asr] {
        for (text, expected) in [
            ("型番ZX-900B", RecognitionDecision::Review),
            ("体系キープ", RecognitionDecision::Undetermined),
        ] {
            let mut input =
                RecognitionInput::new(text, source, "synthetic-clean", "counterexample");
            input.domain = Some("contract_fixture");
            let report = experimental.evaluate_recognition(input).unwrap();
            assert_eq!(report.decision, expected);
            assert_eq!(report.recognition_risk.value, None);
            assert_eq!(report.metrics.slm_calls, 0);
        }
    }
    let changed = RecognitionEngine::new(
        Arc::new(SudachiAnalyzer::new(assets(SudachiMode::A)).unwrap()),
        RecognitionConfig::default(),
    )
    .unwrap()
    .with_statistics(provider);
    let mut input = RecognitionInput::new("猫", RecognitionSource::Asr, "d", "s");
    input.domain = Some("contract_fixture");
    let report = changed.evaluate_recognition(input).unwrap();
    assert!(report
        .reasons
        .contains(&"statistics_analyzer_artifacts_mismatch".into()));
}
