//! Offline scoring only. Never constructs an engine or passes references to inference.
use kaze_nhanh::{RecognitionDecision, RecognitionReport, RecognitionSource};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[path = "recognition_quality/features.rs"]
mod features;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
#[derive(Deserialize)]
struct Reference {
    id: String,
    document_id: Option<String>,
    source: Option<RecognitionSource>,
    text: String,
    transcription: String,
    transcription_status: String,
    comparison_policy: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Observation {
    schema_version: String,
    summary: Value,
    reports: Vec<RecognitionReport>,
}
#[derive(Default)]
struct Counts {
    // Columns: confirmed match, confirmed mismatch, unconfirmed.
    review: [usize; 3],
    undetermined: [usize; 3],
    low_risk: [usize; 3],
}
fn ratio(n: usize, d: usize) -> Option<f64> {
    (d != 0).then(|| n as f64 / d as f64)
}
impl Counts {
    fn add(&mut self, decision: RecognitionDecision, mismatch: Option<bool>) {
        let row = match decision {
            RecognitionDecision::Review => &mut self.review,
            RecognitionDecision::Undetermined => &mut self.undetermined,
            RecognitionDecision::LowRisk => &mut self.low_risk,
        };
        row[match mismatch {
            Some(false) => 0,
            Some(true) => 1,
            None => 2,
        }] += 1;
    }
    fn summary(&self) -> Value {
        let matched = self.review[0] + self.undetermined[0] + self.low_risk[0];
        let mismatched = self.review[1] + self.undetermined[1] + self.low_risk[1];
        let confirmed = matched + mismatched;
        let unconfirmed = self.review[2] + self.undetermined[2] + self.low_risk[2];
        json!({
            "case_count": confirmed + unconfirmed, "confirmed_count": confirmed,
            "unconfirmed_count": unconfirmed, "confirmed_match_count": matched,
            "confirmed_mismatch_count": mismatched,
            "columns": ["confirmed_match", "confirmed_mismatch", "unconfirmed"],
            "decision_counts": {"review": self.review, "undetermined": self.undetermined, "low_risk": self.low_risk},
            "confirmed_metrics": {
                "review_precision": ratio(self.review[1], self.review[0] + self.review[1]),
                "review_recall": ratio(self.review[1], mismatched),
                "review_false_positive_rate": ratio(self.review[0], matched),
                "undetermined_rate": ratio(self.undetermined[0] + self.undetermined[1], confirmed),
                "low_risk_coverage": ratio(self.low_risk[0] + self.low_risk[1], confirmed),
                "low_risk_error_rate": ratio(self.low_risk[1], self.low_risk[0] + self.low_risk[1]),
                "mismatches_not_reviewed": self.undetermined[1] + self.low_risk[1]
            }
        })
    }
}
fn comparison_text<'a>(text: &'a str, policy: &str) -> Result<&'a str> {
    match policy {
        "raw.v1" => Ok(text),
        // Deliberately narrow: no Unicode normalization, whitespace folding or spelling correction.
        "ignore_leading_bullet.v1" => Ok(text.strip_prefix(['・', '·', '•']).unwrap_or(text)),
        _ => Err("unsupported comparison policy".into()),
    }
}
fn confirmed(status: &str) -> Result<bool> {
    match status {
        "user_confirmed_2026-10-04" | "verified" => Ok(true),
        "image_transcription_unconfirmed" | "unconfirmed" => Ok(false),
        _ => Err("unknown transcription status; explicitly map it before scoring".into()),
    }
}
fn score(data: &str, observation: Observation) -> Result<Value> {
    score_with_features(data, observation, false)
}
fn score_with_features(data: &str, observation: Observation, with_features: bool) -> Result<Value> {
    score_decisions(data, observation, with_features, None)
}
fn score_decisions(
    data: &str,
    observation: Observation,
    with_features: bool,
    mut decisions: Option<BTreeMap<String, RecognitionDecision>>,
) -> Result<Value> {
    if observation.schema_version != "kzn.recognition.observation.v1" {
        return Err("unsupported observation schema".into());
    }
    // Summary claims are never trusted as counts.
    let _ = observation.summary;
    let mut reports = BTreeMap::new();
    for report in observation.reports {
        report.validate()?;
        if reports.insert(report.segment_id.clone(), report).is_some() {
            return Err("duplicate report segment ID".into());
        }
    }
    let mut counts = Counts::default();
    let mut features = features::FeatureComparison::default();
    let mut policies = BTreeMap::<String, usize>::new();
    let mut cases = 0;
    for line in data.lines().filter(|s| !s.trim().is_empty()) {
        cases += 1;
        if cases > 4096 {
            return Err("reference case limit exceeded".into());
        }
        // Additional dataset metadata/expected labels are intentionally ignored.
        let reference: Reference = serde_json::from_str(line)?;
        let report = reports
            .remove(&reference.id)
            .ok_or("missing or duplicate reference ID")?;
        if reference.text != report.original_text
            || reference.source.unwrap_or(RecognitionSource::Ocr) != report.source
            || reference.document_id.as_deref().unwrap_or("user-image-001") != report.document_id
        {
            return Err("reference/report text, source or document mismatch".into());
        }
        let policy = match reference.comparison_policy.as_deref().unwrap_or("raw.v1") {
            "ignore_leading_bullet" => "ignore_leading_bullet.v1",
            other => other,
        };
        let mismatch = comparison_text(&reference.text, policy)?
            != comparison_text(&reference.transcription, policy)?;
        *policies.entry(policy.to_owned()).or_default() += 1;
        if with_features {
            features.add(
                &report,
                confirmed(&reference.transcription_status)?.then_some(mismatch),
            )?;
        }
        counts.add(
            match decisions.as_mut() {
                Some(map) => map
                    .remove(&report.segment_id)
                    .ok_or("missing source decision")?,
                None => report.decision,
            },
            confirmed(&reference.transcription_status)?.then_some(mismatch),
        );
    }
    if cases == 0 || !reports.is_empty() || decisions.as_ref().is_some_and(|m| !m.is_empty()) {
        return Err("empty references or unmatched reports".into());
    }
    let mut output = json!({"schema_version":"kzn.recognition.quality_observation.v1",
        "quality_accepted":false, "evaluation_role":"development_observation_not_held_out_test",
        "comparison_policies":policies, "summary":counts.summary(),
        "limitations":["unconfirmed references excluded from quality denominators",
        "segment mismatch only; no CER/WER, calibration or span quality",
        "undetermined is not detection and not low-risk acceptance",
        "no independence or representativeness claimed; no confidence intervals"]});
    if with_features {
        output["feature_ablation"] = features.summary();
    }
    Ok(output)
}
fn read_bounded(path: &std::ffi::OsStr) -> Result<Vec<u8>> {
    if std::fs::metadata(path)?.len() > 64 * 1024 * 1024 {
        return Err("input exceeds 64 MiB".into());
    }
    Ok(std::fs::read(path)?)
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 && !(args.len() == 4 && args[3] == "--feature-ablation") {
        return Err(
            "Usage: recognition_quality references.jsonl observations.json summary.json [--feature-ablation]".into(),
        );
    }
    let data = read_bounded(&args[0])?;
    let reports = read_bounded(&args[1])?;
    let mut output = score_bytes(std::str::from_utf8(&data)?, &reports, args.len() == 4)?;
    output["reference_sha256"] = json!(format!("{:x}", Sha256::digest(&data)));
    output["observation_sha256"] = json!(format!("{:x}", Sha256::digest(&reports)));
    let path = std::path::Path::new(&args[2]);
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(&output)?)?;
    println!("{}", output["summary"]);
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn abstention_is_not_detection_or_acceptance() {
        let mut c = Counts::default();
        c.add(RecognitionDecision::Undetermined, Some(true));
        c.add(RecognitionDecision::Undetermined, Some(false));
        c.add(RecognitionDecision::Review, None);
        let s = c.summary();
        assert_eq!(s["confirmed_count"], 2);
        assert_eq!(s["confirmed_metrics"]["review_recall"], 0.0);
        assert!(s["confirmed_metrics"]["review_precision"].is_null());
        assert!(s["confirmed_metrics"]["low_risk_error_rate"].is_null());
        assert_eq!(s["confirmed_metrics"]["undetermined_rate"], 1.0);
        assert_eq!(s["confirmed_metrics"]["mismatches_not_reviewed"], 1);
    }
    #[test]
    fn mixed_decisions_keep_unknowns_out_of_denominators() {
        let mut c = Counts::default();
        for (d, label) in [
            (RecognitionDecision::Review, Some(true)),
            (RecognitionDecision::Review, Some(false)),
            (RecognitionDecision::LowRisk, Some(true)),
            (RecognitionDecision::LowRisk, Some(false)),
            (RecognitionDecision::Undetermined, Some(true)),
            (RecognitionDecision::LowRisk, None),
        ] {
            c.add(d, label);
        }
        let m = &c.summary()["confirmed_metrics"];
        assert_eq!(m["review_precision"], 0.5);
        assert_eq!(m["review_recall"], 1.0 / 3.0);
        assert_eq!(m["low_risk_coverage"], 0.4);
        assert_eq!(m["low_risk_error_rate"], 0.5);
        assert_eq!(m["mismatches_not_reviewed"], 2);
        assert!(Counts::default().summary()["confirmed_metrics"]["review_recall"].is_null());
    }
    #[test]
    fn comparison_and_confirmation_are_explicit() {
        assert_eq!(
            comparison_text("・体系キープ", "ignore_leading_bullet.v1").unwrap(),
            "体系キープ"
        );
        assert_ne!(comparison_text("過す", "raw.v1").unwrap(), "過ごす");
        assert_ne!(comparison_text("１０kg", "raw.v1").unwrap(), "10kg");
        assert!(comparison_text("text", "fuzzy").is_err());
        assert!(confirmed("looks_confirmed").is_err());
        assert!(!confirmed("image_transcription_unconfirmed").unwrap());
    }
    #[test]
    fn joins_are_exact_and_invalid_reports_cannot_be_scored() {
        use kaze_nhanh::*;
        struct Empty;
        impl MorphAnalyzer for Empty {
            fn analyze(&self, _: &str) -> std::result::Result<MorphAnalysis, EvaluationError> {
                Ok(MorphAnalysis {
                    morphemes: vec![],
                    provenance: vec![],
                })
            }
        }
        let engine =
            RecognitionEngine::new(std::sync::Arc::new(Empty), RecognitionConfig::default())
                .unwrap();
        for source in [RecognitionSource::Ocr, RecognitionSource::Asr] {
            let report = engine
                .evaluate_recognition(RecognitionInput::new("認識", source, "doc", "one"))
                .unwrap();
            let reference = json!({"id":"one","document_id":"doc","source":source,"text":"認識",
                "transcription":"原文","transcription_status":"verified","ocr_mismatch_expected":false});
            let observation = || Observation {
                schema_version: "kzn.recognition.observation.v1".into(),
                summary: json!({"case_count":999}),
                reports: vec![report.clone()],
            };
            let result = score(&reference.to_string(), observation()).unwrap();
            assert_eq!(result["summary"]["confirmed_mismatch_count"], 1);
            for key in ["id", "document_id", "text"] {
                let mut bad = reference.clone();
                bad[key] = json!("wrong");
                assert!(score(&bad.to_string(), observation()).is_err());
            }
            let mut bad = reference.clone();
            bad["source"] = json!(if source == RecognitionSource::Ocr {
                "asr"
            } else {
                "ocr"
            });
            assert!(score(&bad.to_string(), observation()).is_err());
            assert!(score("", observation()).is_err());
            assert!(score(&format!("{reference}\n{reference}"), observation()).is_err());
            let mut bad = observation();
            bad.reports.push(report.clone());
            assert!(score(&reference.to_string(), bad).is_err());
            let mut bad = observation();
            bad.reports[0].schema_version = "unknown".into();
            assert!(score(&reference.to_string(), bad).is_err());
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceObservation {
    schema_version: String,
    reports: Vec<kaze_nhanh::source_adapters::SourceReviewReport>,
}
fn score_bytes(data: &str, bytes: &[u8], with_features: bool) -> Result<Value> {
    let value: Value = serde_json::from_slice(bytes)?;
    match value["schema_version"].as_str() {
        Some("kzn.recognition.source_review_observation.v1") => {
            let source: SourceObservation = serde_json::from_value(value)?;
            let mut decisions = BTreeMap::new();
            let mut reports = Vec::new();
            let mut rules = BTreeMap::<String, Value>::new();
            for report in source.reports {
                report.validate()?;
                if decisions
                    .insert(report.base.segment_id.clone(), report.decision)
                    .is_some()
                {
                    return Err("duplicate source report ID".into());
                }
                let snapshot = serde_json::to_value(&report.rule)?;
                if rules
                    .insert(report.rule.id.clone(), snapshot.clone())
                    .is_some_and(|old| old != snapshot)
                {
                    return Err("same source rule ID has different snapshots".into());
                }
                reports.push(report.base);
            }
            let base = Observation {
                schema_version: "kzn.recognition.observation.v1".into(),
                summary: Value::Null,
                reports: reports.clone(),
            };
            let baseline = score(data, base)?;
            let mut output = score_decisions(
                data,
                Observation {
                    schema_version: "kzn.recognition.observation.v1".into(),
                    summary: Value::Null,
                    reports,
                },
                with_features,
                Some(decisions),
            )?;
            output["decision_scope"] = json!("source_review");
            output["input_schema"] = json!(source.schema_version);
            output["base_summary"] = baseline["summary"].clone();
            output["source_rules"] = json!(rules);
            if with_features {
                output["feature_ablation_scope"] = json!("base_evidence_only");
            }
            Ok(output)
        }
        Some("kzn.recognition.observation.v1") => {
            score_with_features(data, serde_json::from_value(value)?, with_features)
        }
        _ => Err("unsupported observation schema".into()),
    }
}

#[cfg(test)]
mod source_tests {
    use super::*;
    use kaze_nhanh::source_adapters::*;
    use kaze_nhanh::*;
    fn fixture(source: RecognitionSource) -> SourceReviewReport {
        struct Empty;
        impl MorphAnalyzer for Empty {
            fn analyze(&self, _: &str) -> std::result::Result<MorphAnalysis, EvaluationError> {
                Ok(MorphAnalysis {
                    morphemes: vec![],
                    provenance: vec![],
                })
            }
        }
        let score = RawScore {
            value: 0.2,
            meaning: ConfidenceMeaning::EngineScore,
            direction: ConfidenceDirection::HigherIsBetter,
            range: None,
            calibration_id: None,
            target: Some("fixture.score".into()),
        };
        let evidence = RecognizerEvidence {
            schema_version: RECOGNIZER_EVIDENCE_SCHEMA.into(),
            source,
            recognizer: RecognizerIdentity {
                engine: "fixture".into(),
                model: Some("m".into()),
                version: Some("v1".into()),
                decoder: Some("d".into()),
            },
            profile: RecognitionSourceProfile {
                id: "fixture.profile".into(),
                source,
                transcription_policy_id: Some("raw.v1".into()),
                required_signals: vec![],
            },
            confidences: vec![ConfidenceObservation {
                id: "c".into(),
                span: ByteSpan::whole("字"),
                granularity: ConfidenceGranularity::Segment,
                status: RecognitionEvidenceStatus::Observed,
                score: Some(score.clone()),
                aggregation: Some("fixture.mean".into()),
                reason: None,
                dependencies: vec![],
            }],
            candidates: CandidateEvidence::missing(),
            anchors: vec![],
        };
        let rule = ConfidenceReviewRule {
            id: "fixture.rule".into(),
            recognizer: evidence.recognizer.clone(),
            profile: evidence.profile.clone(),
            domain: "fixture".into(),
            granularity: ConfidenceGranularity::Segment,
            aggregation: "fixture.mean".into(),
            threshold: RawScore {
                value: 0.5,
                ..score
            },
        };
        let engine =
            RecognitionEngine::new(std::sync::Arc::new(Empty), RecognitionConfig::default())
                .unwrap();
        let mut input = RecognitionInput::new("字", source, "doc", "one");
        input.domain = Some("fixture");
        input.recognizer_evidence = Some(&evidence);
        rule.evaluate(&engine, input).unwrap()
    }
    fn reference(source: RecognitionSource) -> Value {
        json!({"id":"one","document_id":"doc","source":source,"text":"字","transcription":"文","transcription_status":"verified"})
    }
    fn envelope(reports: Vec<SourceReviewReport>) -> Vec<u8> {
        serde_json::to_vec(&json!({"schema_version":"kzn.recognition.source_review_observation.v1","reports":reports})).unwrap()
    }
    #[test]
    fn scores_final_decision_separately_from_unchanged_base_for_both_sources() {
        for source in [RecognitionSource::Ocr, RecognitionSource::Asr] {
            let report = fixture(source);
            let data = reference(source).to_string();
            let out = score_bytes(&data, &envelope(vec![report]), true).unwrap();
            assert_eq!(
                out["summary"]["decision_counts"]["review"],
                json!([0, 1, 0])
            );
            assert_eq!(
                out["base_summary"]["decision_counts"]["undetermined"],
                json!([0, 1, 0])
            );
            assert_eq!(out["decision_scope"], "source_review");
            assert_eq!(out["feature_ablation_scope"], "base_evidence_only");
            assert_eq!(
                out["source_rules"]["fixture.rule"]["threshold"]["value"],
                0.5
            );
            let mut unconfirmed = reference(source);
            unconfirmed["transcription_status"] = json!("unconfirmed");
            let out = score_bytes(
                &unconfirmed.to_string(),
                &envelope(vec![fixture(source)]),
                false,
            )
            .unwrap();
            assert_eq!(
                out["summary"]["decision_counts"]["review"],
                json!([0, 0, 1])
            );
            assert!(out["summary"]["confirmed_metrics"]["review_precision"].is_null());
        }
    }
    #[test]
    fn rejects_forged_wrapper_duplicate_ids_joins_and_rule_id_collisions() {
        let report = fixture(RecognitionSource::Ocr);
        let data = reference(RecognitionSource::Ocr).to_string();
        let mut bad = report.clone();
        bad.decision = RecognitionDecision::Undetermined;
        assert!(score_bytes(&data, &envelope(vec![bad]), false).is_err());
        assert!(score_bytes(
            &data,
            &envelope(vec![report.clone(), report.clone()]),
            false
        )
        .is_err());
        let mut mismatch = reference(RecognitionSource::Ocr);
        mismatch["text"] = json!("違");
        assert!(score_bytes(
            &mismatch.to_string(),
            &envelope(vec![report.clone()]),
            false
        )
        .is_err());
        let mut rule = report.rule.clone();
        rule.threshold.value = 0.6;
        let mut base = report.base.clone();
        base.segment_id = "two".into();
        let second = rule.combine(base).unwrap();
        assert!(score_bytes(&data, &envelope(vec![report, second]), false).is_err());
        assert!(score_bytes(&data, b"{\"schema_version\":\"unknown\"}", false).is_err());
    }
    #[test]
    fn legacy_cli_dispatch_preserves_existing_output() {
        let report = fixture(RecognitionSource::Ocr).base;
        let data = reference(RecognitionSource::Ocr).to_string();
        let json = json!({"schema_version":"kzn.recognition.observation.v1","summary":{},"reports":[report]});
        let expected = score(&data, serde_json::from_value(json.clone()).unwrap()).unwrap();
        assert_eq!(
            score_bytes(&data, &serde_json::to_vec(&json).unwrap(), false).unwrap(),
            expected
        );
    }
}
