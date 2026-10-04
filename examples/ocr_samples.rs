//! Observation runner. Default primary-only; optional explicit offline model comparison.
use kaze_nhanh::*;
use serde::Deserialize;
use std::path::PathBuf;
#[cfg(feature = "qwen")]
use std::{
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
    #[serde(default = "legacy_document")]
    document_id: String,
    #[serde(default)]
    image_column_from_right: Option<usize>,
    #[serde(default)]
    image_position: serde_json::Value,
    transcription: Option<String>,
    transcription_status: String,
    #[serde(default)]
    transcription_candidates: Vec<String>,
    #[serde(default = "exact")]
    comparison_policy: String,
    ocr_mismatch_expected: Option<bool>,
    #[serde(default)]
    difference_kind: Option<String>,
    #[serde(default)]
    note: Option<String>,
}
fn legacy_document() -> String {
    "user-image-001".into()
}
fn exact() -> String {
    "exact".into()
}
fn comparable<'a>(text: &'a str, policy: &str) -> Result<&'a str, &'static str> {
    match policy {
        "exact" => Ok(text),
        "ignore_leading_bullet" => Ok(text.trim_start_matches(['・', '·']).trim_start()),
        _ => Err("unknown fidelity comparison policy"),
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if args.len() < 2 || args.len() > 3 || args.len() == 3 && args[2] != "--compare-qwen" {
        return Err("Usage: ocr_samples fixture.jsonl output.json [--compare-qwen]".into());
    }
    let compare = args.len() == 3;
    #[cfg(not(feature = "qwen"))]
    if compare {
        return Err("--compare-qwen requires --features qwen".into());
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&args[0])?;
    let samples = text
        .lines()
        .filter(|s| !s.trim().is_empty())
        .map(serde_json::from_str::<Sample>)
        .collect::<Result<Vec<_>, _>>()?;
    let mut ids = std::collections::HashSet::new();
    if samples.is_empty() {
        return Err("empty OCR dataset".into());
    }
    for s in &samples {
        if !ids.insert(&s.id)
            || !s.confidence.is_finite()
            || !(0.0..=1.0).contains(&s.confidence)
            || s.document_id.is_empty()
        {
            return Err("invalid sample identity/confidence/document".into());
        }
        if let Some(reference) = &s.transcription {
            if s.ocr_mismatch_expected
                != Some(
                    comparable(&s.text, &s.comparison_policy)?
                        != comparable(reference, &s.comparison_policy)?,
                )
            {
                return Err("inconsistent fidelity label".into());
            }
        } else if s.ocr_mismatch_expected.is_some() {
            return Err("ambiguous transcription cannot have definitive fidelity label".into());
        }
    }
    let profile = DomainProfile::builtin("ja.ocr.v1")?;
    let engine = japanese_engine_with_profile(
        SudachiConfig::from_paths(
            root.join("resources/sudachi/system.dic"),
            root.join("resources/sudachi/sudachi.json"),
            SudachiMode::C,
        )?,
        profile.clone(),
        profile.evaluation_config(),
    )?;
    #[cfg(feature = "qwen")]
    let factory = if compare {
        Some(Arc::new(QwenNaturalnessFactory::new(
            QwenJudgeConfig::local(root.join("target/qwen-judge")),
        )?))
    } else {
        None
    };
    #[cfg(feature = "qwen")]
    let worker = factory
        .as_ref()
        .map(|f| {
            SecondaryWorker::new(
                f.clone(),
                SecondaryPolicy {
                    timeout: Duration::from_secs(180),
                    max_calls: samples.len(),
                    ..SecondaryPolicy::default()
                },
            )
            .map(Arc::new)
        })
        .transpose()?;
    #[cfg(feature = "qwen")]
    let engine = if let Some(worker) = &worker {
        engine.with_secondary_worker(worker.clone())
    } else {
        engine
    };
    #[cfg(feature = "qwen")]
    let mut comparison = factory
        .as_ref()
        .map(|f| {
            f.load_local(Instant::now() + Duration::from_secs(180))
                .map_err(|e| format!("comparison load: {e:?}"))
        })
        .transpose()?;
    let labels_confirmed = samples
        .iter()
        .all(|s| s.transcription_status.starts_with("user_confirmed_"));
    let mut results = Vec::new();
    let mut missed = 0;
    let mut mismatches = 0;
    let mut unknown = 0;
    let mut confirmed_missed = 0;
    for sample in samples {
        let annotations = [SourceAnnotation {
            span: ByteSpan::whole(&sample.text),
            data: serde_json::json!({"ocr_engine":sample.engine,"ocr_confidence":sample.confidence,"image_column_from_right":sample.image_column_from_right,"image_position":sample.image_position,"document_id":sample.document_id,"confidence_is_calibrated":false}),
        }];
        let mut input = TextInput::new(&sample.text);
        input.source = SourceKind::Ocr;
        input.annotations = &annotations;
        let report = engine.evaluate(input)?;
        report.validate()?;
        if report.original_text != sample.text || report.annotations != annotations {
            return Err("OCR source information changed".into());
        }
        let gate_miss = sample.ocr_mismatch_expected == Some(true)
            && report.routing.secondary_needed != Some(true)
            && report.verdict != Verdict::Invalid;
        mismatches += usize::from(sample.ocr_mismatch_expected == Some(true));
        unknown += usize::from(sample.ocr_mismatch_expected.is_none());
        missed += usize::from(gate_miss);
        confirmed_missed +=
            usize::from(gate_miss && sample.transcription_status.starts_with("user_confirmed_"));
        let offline = serde_json::json!({"status":"not_requested"});
        #[cfg(feature = "qwen")]
        let offline = if let Some(judge) = &mut comparison {
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
            match judge.judge(&request) {
                Ok(result) => {
                    serde_json::json!({"status":"completed","elapsed_ms":start.elapsed().as_millis(),"evaluation":result})
                }
                Err(e) => {
                    serde_json::json!({"status":format!("{e:?}"),"elapsed_ms":start.elapsed().as_millis()})
                }
            }
        } else {
            offline
        };
        results.push(serde_json::json!({"id":sample.id,"document_id":sample.document_id,"transcription":sample.transcription,"transcription_status":sample.transcription_status,"transcription_candidates":sample.transcription_candidates,"comparison_policy":sample.comparison_policy,"difference_kind":sample.difference_kind,"note":sample.note,"ocr_mismatch_expected":sample.ocr_mismatch_expected,"cascade":report,"offline_all_input_comparison":offline}));
    }
    let forwards = 0;
    let attempts = 0;
    #[cfg(feature = "qwen")]
    let forwards = factory.as_ref().map_or(forwards, |f| f.total_forwards());
    #[cfg(feature = "qwen")]
    let attempts = worker.as_ref().map_or(attempts, |w| w.total_calls());
    let output = serde_json::json!({"dataset":PathBuf::from(&args[0]).file_stem().unwrap().to_string_lossy(),"labels_confirmed":labels_confirmed,"expected_mismatches":mismatches,"unresolved_transcriptions":unknown,"gate_missed_candidates":missed,"confirmed_gate_missed_candidates":confirmed_missed,"cascade_judge_attempts":attempts,"model_forward_total":forwards,"model_comparison_requested":compare,"results":results});
    let path = PathBuf::from(&args[1]);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(&output)?)?;
    println!(
        "{}",
        serde_json::to_string(
            &serde_json::json!({"expected_mismatches":mismatches,"unresolved_transcriptions":unknown,"gate_missed_candidates":missed,"total_forwards":forwards})
        )?
    );
    Ok(())
}
