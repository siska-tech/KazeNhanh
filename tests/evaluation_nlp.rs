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
        assert_eq!(report.verdict, Verdict::Undetermined);
        assert_eq!(report.metrics.slm_calls, 0);
        assert!(report.metrics.morpheme_count > 3);
        assert!(report.scores.naturalness.value.is_none());
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
