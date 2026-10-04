//! Reproducible hand-authored primary fixtures; not a calibrated quality dataset.
use kaze_nhanh::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, error::Error, path::PathBuf, sync::Arc, time::Instant};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    text: String,
    source: SourceKind,
    profile: String,
    category: String,
    expected_verdict: Verdict,
    expected_codes: Vec<String>,
    reference: Option<String>,
    profile_override: Option<DomainProfile>,
}
#[derive(Serialize)]
struct Summary {
    fixture_sha256: String,
    rules_id: String,
    case_count: usize,
    matched_cases: usize,
    normal_count: usize,
    normal_false_alarms: usize,
    semantic_hold_count: usize,
    slm_calls: usize,
    dictionary_load_ms: u128,
    p50_us: u128,
    p95_us: u128,
    failures: Vec<String>,
}
fn main() -> Result<(), Box<dyn Error>> {
    let args = std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    if args.len() != 2 {
        return Err("Usage: primary_baseline fixture.jsonl output-directory".into());
    }
    let bytes = std::fs::read(&args[0])?;
    let text = std::str::from_utf8(&bytes)?;
    let cases = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str::<Case>)
        .collect::<Result<Vec<_>, _>>()?;
    let mut ids = HashSet::new();
    if cases.is_empty() || cases.iter().any(|case| !ids.insert(&case.id)) {
        return Err("Fixture must be nonempty with unique IDs".into());
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let load = Instant::now();
    let analyzer = Arc::new(SudachiAnalyzer::new(SudachiConfig::from_paths(
        root.join("resources/sudachi/system.dic"),
        root.join("resources/sudachi/sudachi.json"),
        SudachiMode::C,
    )?)?);
    let mut summary = Summary {
        fixture_sha256: format!("{:x}", Sha256::digest(&bytes)),
        rules_id: PRIMARY_RULES_ID.into(),
        case_count: cases.len(),
        matched_cases: 0,
        normal_count: 0,
        normal_false_alarms: 0,
        semantic_hold_count: 0,
        slm_calls: 0,
        dictionary_load_ms: load.elapsed().as_millis(),
        p50_us: 0,
        p95_us: 0,
        failures: vec![],
    };
    let mut timings = Vec::new();
    let mut output = String::new();
    for case in cases {
        let profile = match case.profile_override {
            Some(profile) => profile,
            None => DomainProfile::builtin(&case.profile)?,
        };
        if profile.id != case.profile {
            return Err(format!("profile ID mismatch: {}", case.id).into());
        }
        let config = profile.evaluation_config();
        let engine = EvaluationEngine::new(analyzer.clone(), config)?
            .with_primary_detector(Arc::new(PrimaryRules::new(profile)?));
        let mut input = TextInput::new(&case.text);
        input.source = case.source;
        input.reference = case.reference.as_deref();
        let start = Instant::now();
        let report = engine.evaluate(input)?;
        let elapsed_us = start.elapsed().as_micros();
        timings.push(elapsed_us);
        report.validate()?;
        let mut actual = report
            .issues
            .iter()
            .map(|issue| issue.code.clone())
            .collect::<Vec<_>>();
        actual.sort();
        actual.dedup();
        let mut expected = case.expected_codes;
        expected.sort();
        expected.dedup();
        let matches = report.verdict == case.expected_verdict
            && actual == expected
            && report.metrics.slm_calls == 0;
        if matches {
            summary.matched_cases += 1;
        } else {
            summary.failures.push(format!(
                "{}: expected {:?}/{expected:?}, actual {:?}/{actual:?}",
                case.id, case.expected_verdict, report.verdict
            ));
        }
        if case.category == "normal" {
            summary.normal_count += 1;
            summary.normal_false_alarms += usize::from(matches!(
                report.verdict,
                Verdict::Suspicious | Verdict::Invalid
            ));
        }
        if case.category == "semantic_hold" {
            summary.semantic_hold_count += 1;
            if report.scores.semantic_consistency.value.is_some()
                || report.routing.secondary_needed != Some(true)
            {
                summary
                    .failures
                    .push(format!("{}: semantic hold violated", case.id));
            }
        }
        summary.slm_calls += report.metrics.slm_calls;
        output.push_str(&serde_json::to_string(&serde_json::json!({"id":case.id,"category":case.category,"elapsed_us":elapsed_us,"matches_fixture":matches,"report":report}))?);
        output.push('\n');
    }
    timings.sort();
    summary.p50_us = timings[(timings.len() - 1) / 2];
    summary.p95_us = timings[(timings.len() * 95).div_ceil(100).saturating_sub(1)];
    std::fs::create_dir_all(&args[1])?;
    std::fs::write(args[1].join("reports.jsonl"), output)?;
    let json = serde_json::to_string_pretty(&summary)?;
    std::fs::write(args[1].join("summary.json"), &json)?;
    println!("{json}");
    if !summary.failures.is_empty() {
        return Err("Primary fixture regression: see summary.json".into());
    }
    Ok(())
}
