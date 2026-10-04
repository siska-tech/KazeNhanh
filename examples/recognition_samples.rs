//! Model-free observation runner for user-provided OCR. Gold/reference fields are never read.
use kaze_nhanh::source_adapters::*;
use kaze_nhanh::*;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if args.len() != 2 {
        return Err("Usage: recognition_samples ocr.jsonl output.json".into());
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let assets = SudachiConfig::from_paths(
        root.join("resources/sudachi/system.dic"),
        root.join("resources/sudachi/sudachi.json"),
        SudachiMode::C,
    )?;
    let engine = japanese_recognition_engine(assets, RecognitionConfig::default())?;
    let data = std::fs::read_to_string(&args[0])?;
    let mut ids = std::collections::HashSet::new();
    let mut reports = Vec::new();
    for line in data.lines().filter(|line| !line.trim().is_empty()) {
        let sample: serde_json::Value = serde_json::from_str(line)?;
        let text = sample["text"].as_str().ok_or("text is required")?;
        let id = sample["id"].as_str().ok_or("id is required")?;
        let engine_id = sample["engine"].as_str().ok_or("engine is required")?;
        let raw = sample["confidence"]
            .as_f64()
            .ok_or("numeric confidence is required")?;
        if !ids.insert(id.to_owned()) {
            return Err("duplicate sample identity".into());
        }
        let document = sample["document_id"].as_str().unwrap_or("user-image-001");
        let mut profile =
            RecognitionSourceProfile::new("ja.ocr.user-segment.v1", RecognitionSource::Ocr);
        profile.required_signals = vec![RecognizerSignal::Confidence];
        let evidence = adapt_ocr_evidence(
            text,
            OcrEvidencePayload {
                recognizer: RecognizerIdentity {
                    engine: engine_id.into(),
                    model: None,
                    version: None,
                    decoder: None,
                },
                profile,
                confidences: vec![ConfidenceObservation {
                    id: "recognizer-segment-confidence".into(),
                    span: ByteSpan::whole(text),
                    granularity: ConfidenceGranularity::Segment,
                    status: RecognitionEvidenceStatus::Observed,
                    score: Some(RawScore {
                        value: raw,
                        meaning: ConfidenceMeaning::EngineScore,
                        direction: ConfidenceDirection::Unknown,
                        range: None,
                        calibration_id: None,
                        target: None,
                    }),
                    aggregation: None,
                    reason: None,
                    dependencies: vec!["recognizer-decoder".into()],
                }],
                candidates: CandidateEvidence::missing(),
                regions: vec![],
            },
        )?;
        let annotations = [SourceAnnotation {
            span: ByteSpan::whole(text),
            data: serde_json::json!({"ocr_engine":engine_id,"ocr_confidence":raw,"document_id":document,
                "image_position":sample["image_position"],"image_column_from_right":sample["image_column_from_right"]}),
        }];
        let mut input = RecognitionInput::new(text, RecognitionSource::Ocr, document, id);
        input.annotations = &annotations;
        input.recognizer_evidence = Some(&evidence);
        let report = engine.evaluate_recognition(input)?;
        report.validate()?;
        if report.original_text != text
            || report.annotations != annotations
            || report.recognizer_evidence.as_ref() != Some(&evidence)
            || report.metrics.slm_calls != 0
            || report.recognition_risk.value.is_some()
            || report.decision == RecognitionDecision::LowRisk
        {
            return Err("recognition observation contract violation".into());
        }
        reports.push(report);
    }
    if reports.is_empty() {
        return Err("empty recognition dataset".into());
    }
    let summary = serde_json::json!({"case_count":reports.len(),
        "undetermined_count":reports.iter().filter(|r| r.decision == RecognitionDecision::Undetermined).count(),
        "review_count":reports.iter().filter(|r| r.decision == RecognitionDecision::Review).count(),
        "low_risk_count":0,"slm_calls":0,"gold_used_for_inference":false,
        "score_semantics":"uncalibrated_engine_score_direction_and_aggregation_unknown"});
    let output = serde_json::json!({"schema_version":"kzn.recognition.observation.v1","summary":summary,"reports":reports});
    let path = std::path::PathBuf::from(&args[1]);
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(&output)?)?;
    println!("{summary}");
    Ok(())
}
