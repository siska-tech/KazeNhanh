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
            report.decision,
            confirmed(&reference.transcription_status)?.then_some(mismatch),
        );
    }
    if cases == 0 || !reports.is_empty() {
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
    let mut output = if args.len() == 4 {
        score_with_features(
            std::str::from_utf8(&data)?,
            serde_json::from_slice(&reports)?,
            true,
        )?
    } else {
        score(
            std::str::from_utf8(&data)?,
            serde_json::from_slice(&reports)?,
        )?
    };
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
