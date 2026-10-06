//! Model-free observation runner for user-provided OCR. Gold/reference fields are never read.
use kaze_nhanh::source_adapters::*;
use kaze_nhanh::*;
#[path = "recognition_samples/description.rs"]
mod description;
fn confidence(sample: &serde_json::Value) -> Result<Option<f64>, &'static str> {
    match sample.get("confidence") {
        Some(serde_json::Value::Null) => Ok(None),
        Some(value) => value
            .as_f64()
            .filter(|v| v.is_finite())
            .map(Some)
            .ok_or("confidence must be finite numeric or null"),
        None => Err("confidence field is required; use null for missing"),
    }
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceMapping {
    expected_engine: String,
    expected_scale: String,
    rule: ConfidenceReviewRule,
}
impl SourceMapping {
    fn validate_sample(&self, sample: &serde_json::Value) -> Result<(), &'static str> {
        if sample["engine"].as_str() != Some(self.expected_engine.as_str())
            || sample["confidence_scale"].as_str() != Some(self.expected_scale.as_str())
            || sample["source"].as_str() != Some("ocr")
        {
            return Err("source mapping engine/scale/source mismatch");
        }
        Ok(())
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1).collect::<Vec<_>>();
    let timing_path = if args.len() >= 2 && args[args.len() - 2] == "--timings" {
        let path = args.pop().expect("timing path");
        args.pop();
        Some(std::path::PathBuf::from(path))
    } else {
        None
    };
    let mut call_times = Vec::new();
    let sparse_review = args.last().is_some_and(|a| a == "--sparse-review");
    if sparse_review {
        args.pop();
    }
    let dictionary_path = if args.len() >= 2 && args[args.len() - 2] == "--dictionary" {
        let path = std::path::PathBuf::from(args.pop().expect("path argument"));
        args.pop();
        Some(path)
    } else {
        None
    };
    let source_description: Option<description::Description> =
        if args.len() >= 2 && args[args.len() - 2] == "--source-description" {
            let path = args.pop().expect("description path");
            args.pop();
            if std::fs::metadata(&path)?.len() > 65536 {
                return Err("description exceeds 64 KiB".into());
            }
            let d: description::Description = serde_json::from_slice(&std::fs::read(path)?)?;
            d.validate()?;
            Some(d)
        } else {
            None
        };
    let source_mapping: Option<SourceMapping> =
        if args.len() >= 2 && args[args.len() - 2] == "--source-rule" {
            let path = args.pop().expect("source rule path");
            args.pop();
            if std::fs::metadata(&path)?.len() > 65536 {
                return Err("source rule exceeds 64 KiB".into());
            }
            let mapping: SourceMapping = serde_json::from_slice(&std::fs::read(path)?)?;
            mapping.rule.validate()?;
            if mapping.rule.profile.source != RecognitionSource::Ocr {
                return Err("OCR source rule required".into());
            }
            Some(mapping)
        } else {
            None
        };
    if source_description.is_some() && source_mapping.is_some() {
        return Err("source description and rule are mutually exclusive".into());
    }
    if args.len() != 2 && args.len() != 5 {
        return Err("Usage: recognition_samples ocr.jsonl output.json [statistics.json expected-sha256 domain] [--source-rule mapping.json | --source-description description.json] [--dictionary path] [--sparse-review] [--timings timing.json]".into());
    }
    if timing_path
        .as_ref()
        .is_some_and(|p| p == std::path::Path::new(&args[1]))
    {
        return Err("timing and report paths must differ".into());
    }
    let load_start = std::time::Instant::now();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let assets = SudachiConfig::from_paths(
        dictionary_path.unwrap_or_else(|| root.join("resources/sudachi/system.dic")),
        root.join("resources/sudachi/sudachi.json"),
        SudachiMode::C,
    )?;
    let mut engine = japanese_recognition_engine(assets, RecognitionConfig::default())?;
    let statistics_domain = if args.len() == 5 {
        use sha2::{Digest, Sha256};
        if std::fs::metadata(&args[2])?.len() > 8 * 1024 * 1024 {
            return Err("statistics asset exceeds 8 MiB".into());
        }
        let bytes = std::fs::read(&args[2])?;
        let hash = format!("{:x}", Sha256::digest(&bytes));
        if args[3].to_str() != Some(hash.as_str()) {
            return Err("statistics asset SHA256 mismatch".into());
        }
        let artifact: StatisticsArtifact = serde_json::from_slice(&bytes)?;
        engine = engine.with_statistics(std::sync::Arc::new(LightweightStatistics::new(artifact)?));
        Some(args[4].to_str().ok_or("domain must be Unicode")?)
    } else {
        None
    };
    if sparse_review {
        if statistics_domain.is_none() {
            return Err("--sparse-review requires a statistics asset and domain".into());
        }
        engine = engine.with_sparse_statistics_review();
    }
    let load_ms = load_start.elapsed().as_secs_f64() * 1000.0;
    if std::fs::metadata(&args[0])?.len() > 16 * 1024 * 1024 {
        return Err("dataset exceeds 16 MiB".into());
    }
    let data = std::fs::read_to_string(&args[0])?;
    let mut ids = std::collections::HashSet::new();
    let mut reports = Vec::new();
    let mut source_reports = Vec::new();
    if let Some(mapping) = &source_mapping {
        if statistics_domain.is_some_and(|d| d != mapping.rule.domain) {
            return Err("source/statistics domain mismatch".into());
        }
    }
    for line in data.lines().filter(|line| !line.trim().is_empty()) {
        if reports.len() >= 4096 {
            return Err("dataset exceeds 4096 segments".into());
        }
        let sample: serde_json::Value = serde_json::from_str(line)?;
        let text = sample["text"].as_str().ok_or("text is required")?;
        let id = sample["id"].as_str().ok_or("id is required")?;
        let engine_id = sample["engine"].as_str().ok_or("engine is required")?;
        let raw = confidence(&sample)?;
        if let Some(mapping) = &source_mapping {
            mapping.validate_sample(&sample)?;
        }
        if !ids.insert(id.to_owned()) {
            return Err("duplicate sample identity".into());
        }
        let document = sample["document_id"].as_str().unwrap_or("user-image-001");
        let mut profile =
            RecognitionSourceProfile::new("ja.ocr.user-segment.v1", RecognitionSource::Ocr);
        profile.required_signals = vec![RecognizerSignal::Confidence];
        let mut evidence = adapt_ocr_evidence(
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
                    status: if raw.is_some() {
                        RecognitionEvidenceStatus::Observed
                    } else {
                        RecognitionEvidenceStatus::Missing
                    },
                    score: raw.map(|value| RawScore {
                        value,
                        meaning: ConfidenceMeaning::EngineScore,
                        direction: ConfidenceDirection::Unknown,
                        range: None,
                        calibration_id: None,
                        target: None,
                    }),
                    aggregation: None,
                    reason: raw
                        .is_none()
                        .then(|| "recognizer_confidence_not_provided".into()),
                    dependencies: vec!["recognizer-decoder".into()],
                }],
                candidates: CandidateEvidence::missing(),
                regions: vec![],
            },
        )?;
        if let Some(mapping) = &source_mapping {
            evidence.recognizer = mapping.rule.recognizer.clone();
            evidence.profile = mapping.rule.profile.clone();
            let c = &mut evidence.confidences[0];
            c.granularity = mapping.rule.granularity;
            c.aggregation = Some(mapping.rule.aggregation.clone());
            c.score = raw.map(|value| RawScore {
                value,
                ..mapping.rule.threshold.clone()
            });
            evidence.validate(text, RecognitionSource::Ocr)?;
        }
        if let Some(d) = &source_description {
            if statistics_domain.is_some_and(|domain| domain != d.domain) {
                return Err("source/statistics domain mismatch".into());
            }
            evidence = d.apply(&sample, text, raw)?;
        }
        let annotations = [SourceAnnotation {
            span: ByteSpan::whole(text),
            data: serde_json::json!({"ocr_engine":engine_id,"ocr_confidence":raw,"document_id":document,
                "confidence_scale":sample["confidence_scale"],"image_position":sample["image_position"],"image_column_from_right":sample["image_column_from_right"]}),
        }];
        let mut input = RecognitionInput::new(text, RecognitionSource::Ocr, document, id);
        input.domain = statistics_domain
            .or_else(|| source_mapping.as_ref().map(|m| m.rule.domain.as_str()))
            .or_else(|| source_description.as_ref().map(|d| d.domain.as_str()));
        input.annotations = &annotations;
        input.recognizer_evidence = Some(&evidence);
        let call_start = std::time::Instant::now();
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
        if let Some(mapping) = &source_mapping {
            let combined = mapping.rule.combine(report.clone())?;
            combined.validate()?;
            source_reports.push(combined);
        }
        if timing_path.is_some() {
            call_times.push(serde_json::json!({"id":id,"empty":text.is_empty(),"scalar_count":text.chars().count(),"elapsed_ms":call_start.elapsed().as_secs_f64()*1000.0}));
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
        "statistics_enabled":statistics_domain.is_some(),"sparse_review_enabled":sparse_review,
        "score_semantics":if source_mapping.is_some() || source_description.is_some() {"uncalibrated_explicit_source_mapping"} else {"uncalibrated_engine_score_direction_and_aggregation_unknown"}});
    let output = if source_mapping.is_some() {
        serde_json::json!({"schema_version":"kzn.recognition.source_review_observation.v1","reports":source_reports})
    } else {
        serde_json::json!({"schema_version":"kzn.recognition.observation.v1","summary":summary,"reports":reports})
    };
    let path = std::path::PathBuf::from(&args[1]);
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(&output)?)?;
    if let Some(path) = timing_path {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        use sha2::{Digest, Sha256};
        let artifact = serde_json::json!({"schema_version":"kzn.recognition.timing.v1","build":if cfg!(debug_assertions){"debug"}else{"release"},"scope":"engine evaluation + report validation + source combination/validation; excludes input adaptation and serialization; no logistic margin scoring","load_scope":"Sudachi config/hash + engine + statistics read/hash/load","load_ms":load_ms,"input_sha256":format!("{:x}",Sha256::digest(data.as_bytes())),"source_enabled":source_mapping.is_some(),"slm_calls":0,"calls":call_times});
        std::fs::write(path, serde_json::to_vec_pretty(&artifact)?)?;
    }
    if source_mapping.is_some() {
        println!(
            "{}",
            serde_json::json!({"decision_scope":"base","base_summary":summary})
        );
    } else {
        println!("{summary}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn null_is_missing_zero_is_observed_and_malformed_values_fail() {
        assert_eq!(
            confidence(&serde_json::json!({"confidence":null})),
            Ok(None)
        );
        assert_eq!(
            confidence(&serde_json::json!({"confidence":0})),
            Ok(Some(0.0))
        );
        assert_eq!(
            confidence(&serde_json::json!({"confidence":0.99})),
            Ok(Some(0.99))
        );
        for value in [
            serde_json::json!({}),
            serde_json::json!({"confidence":"0.9"}),
            serde_json::json!({"confidence":false}),
        ] {
            assert!(confidence(&value).is_err());
        }
    }
}

#[cfg(test)]
mod source_mapping_tests {
    use super::*;
    #[test]
    fn only_explicit_engine_scale_and_source_bind_to_rule() {
        let rule = ConfidenceReviewRule {
            id: "fixture.rule".into(),
            recognizer: RecognizerIdentity {
                engine: "engine".into(),
                model: Some("m".into()),
                version: Some("v".into()),
                decoder: Some("d".into()),
            },
            profile: RecognitionSourceProfile {
                id: "fixture".into(),
                source: RecognitionSource::Ocr,
                transcription_policy_id: Some("raw.v1".into()),
                required_signals: vec![],
            },
            domain: "fixture".into(),
            granularity: ConfidenceGranularity::Segment,
            aggregation: "fixture.mean".into(),
            threshold: RawScore {
                value: 0.5,
                meaning: ConfidenceMeaning::EngineScore,
                direction: ConfidenceDirection::HigherIsBetter,
                range: Some([0.0, 1.0]),
                calibration_id: None,
                target: Some("token.mean".into()),
            },
        };
        let mapping = SourceMapping {
            expected_engine: "declared.engine".into(),
            expected_scale: "declared.scale".into(),
            rule,
        };
        mapping.rule.validate().unwrap();
        let sample = serde_json::json!({"engine":"declared.engine","confidence_scale":"declared.scale","source":"ocr","confidence":null});
        mapping.validate_sample(&sample).unwrap();
        for key in ["engine", "confidence_scale", "source"] {
            let mut wrong = sample.clone();
            wrong[key] = serde_json::json!("unknown");
            assert!(mapping.validate_sample(&wrong).is_err());
            wrong.as_object_mut().unwrap().remove(key);
            assert!(mapping.validate_sample(&wrong).is_err());
        }
    }
}
