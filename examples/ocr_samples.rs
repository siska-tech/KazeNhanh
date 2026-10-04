//! User-provided OCR observation runner. Labels are provisional, not acceptance gates.
use kaze_nhanh::*;
use serde::Deserialize;
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Sample {
    id: String,
    text: String,
    confidence: f64,
    engine: String,
    image_column_from_right: usize,
    transcription: String,
    transcription_status: String,
    ocr_mismatch_expected: bool,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if args.len() != 2 {
        return Err("Usage: ocr_samples fixture.jsonl output.json".into());
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&args[0])?;
    let samples = text
        .lines()
        .filter(|s| !s.trim().is_empty())
        .map(serde_json::from_str::<Sample>)
        .collect::<Result<Vec<_>, _>>()?;
    let mut ids = std::collections::HashSet::new();
    if samples.is_empty()
        || samples.iter().any(|s| {
            !ids.insert(&s.id)
                || !s.confidence.is_finite()
                || !(0.0..=1.0).contains(&s.confidence)
                || s.ocr_mismatch_expected != (s.text != s.transcription)
        })
    {
        return Err("invalid OCR sample identity/confidence/fidelity label".into());
    }
    let profile = DomainProfile::builtin("ja.ocr.v1")?;
    let factory = Arc::new(QwenNaturalnessFactory::new(QwenJudgeConfig::local(
        root.join("target/qwen-judge"),
    ))?);
    let worker = Arc::new(SecondaryWorker::new(
        factory.clone(),
        SecondaryPolicy {
            timeout: Duration::from_secs(180),
            max_calls: 10,
            ..SecondaryPolicy::default()
        },
    )?);
    let engine = japanese_engine_with_profile(
        SudachiConfig::from_paths(
            root.join("resources/sudachi/system.dic"),
            root.join("resources/sudachi/sudachi.json"),
            SudachiMode::C,
        )?,
        profile.clone(),
        profile.evaluation_config(),
    )?
    .with_secondary_worker(worker.clone());
    // Separate offline all-input comparison, never attached to the production engine.
    let mut comparison = factory
        .load_local(Instant::now() + Duration::from_secs(180))
        .map_err(|e| format!("comparison load: {e:?}"))?;
    let labels_confirmed = samples
        .iter()
        .all(|s| s.transcription_status == "user_confirmed_2026-10-04");
    let mut results = Vec::new();
    let mut missed_candidates = 0;
    let mut expected_mismatches = 0;
    for sample in samples {
        let annotation = SourceAnnotation {
            span: ByteSpan::whole(&sample.text),
            data: serde_json::json!({"ocr_engine":sample.engine,"ocr_confidence":sample.confidence,"image_column_from_right":sample.image_column_from_right,"document_id":"user-image-001","confidence_is_calibrated":false}),
        };
        let annotations = [annotation];
        let mut input = TextInput::new(&sample.text);
        input.source = SourceKind::Ocr;
        input.annotations = &annotations;
        // Never leak image transcription/expected label into detector or judge input.
        let report = engine.evaluate(input)?;
        report.validate()?;
        if report.original_text != sample.text || report.annotations != annotations {
            return Err("OCR source information changed".into());
        }
        expected_mismatches += usize::from(sample.ocr_mismatch_expected);
        missed_candidates += usize::from(
            sample.ocr_mismatch_expected
                && report.routing.secondary_needed != Some(true)
                && report.verdict != Verdict::Invalid,
        );
        let request = SecondaryRequest {
            text: sample.text.clone(),
            reference: None,
            profile_id: profile.id.clone(),
            primary_issues: vec![],
            dimensions: vec![Dimension::Naturalness],
            semantic_scope: ScoreScope::Internal,
            deadline: Instant::now() + Duration::from_secs(180),
            max_generated_tokens: 1,
        };
        let start = Instant::now();
        let judged = comparison.judge(&request);
        let offline = match judged {
            Ok(result) => {
                serde_json::json!({"status":"completed","elapsed_ms":start.elapsed().as_millis(),"evaluation":result})
            }
            Err(e) => {
                serde_json::json!({"status":format!("{e:?}"),"elapsed_ms":start.elapsed().as_millis()})
            }
        };
        results.push(serde_json::json!({"id":sample.id,"transcription":sample.transcription,"transcription_status":sample.transcription_status,"ocr_mismatch_expected":sample.ocr_mismatch_expected,"cascade":report,"offline_all_input_comparison":offline}));
    }
    let output = serde_json::json!({"dataset":"ppocrv6-medium.user-001","labels_confirmed":labels_confirmed,"expected_mismatches":expected_mismatches,"gate_missed_candidates":missed_candidates,"cascade_judge_attempts":worker.total_calls(),"all_input_forward_total":factory.total_forwards(),"results":results});
    let path = PathBuf::from(&args[1]);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(&output)?)?;
    println!(
        "{}",
        serde_json::to_string(
            &serde_json::json!({"expected_mismatches":expected_mismatches,"gate_missed_candidates":missed_candidates,"cascade_judge_attempts":worker.total_calls(),"total_forwards":factory.total_forwards()})
        )?
    );
    Ok(())
}
