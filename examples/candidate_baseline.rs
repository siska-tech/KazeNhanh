//! Synthetic operational-policy comparison. Expectations are offline assertions, never input evidence.
use kaze_nhanh::*;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{collections::HashSet, path::Path, sync::Arc};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    source: RecognitionSource,
    text: String,
    candidates: Option<Vec<String>>,
    baseline: RecognitionDecision,
    candidate_review: RecognitionDecision,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 {
        return Err("Usage: candidate_baseline dictionary.dic contract.jsonl output.json".into());
    }
    if std::fs::metadata(&args[1])?.len() > 1024 * 1024 {
        return Err("contract fixture exceeds 1 MiB".into());
    }
    let bytes = std::fs::read(&args[1])?;
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let analyzer = Arc::new(SudachiAnalyzer::new(SudachiConfig::from_paths(
        &args[0],
        root.join("resources/sudachi/sudachi.json"),
        SudachiMode::C,
    )?)?);
    let baseline = RecognitionEngine::new(analyzer.clone(), RecognitionConfig::default())?;
    let candidate_review = RecognitionEngine::new(analyzer, RecognitionConfig::default())?
        .with_candidate_disagreement_review();
    let mut ids = HashSet::new();
    let mut output = Vec::new();
    for line in std::str::from_utf8(&bytes)?
        .lines()
        .filter(|l| !l.trim().is_empty())
    {
        if output.len() >= 256 {
            return Err("too many contract cases".into());
        }
        let case: Case = serde_json::from_str(line)?;
        if case.id.trim().is_empty() || !ids.insert(case.id.clone()) {
            return Err("invalid/duplicate case ID".into());
        }
        let mut candidates = match case.candidates {
            None => CandidateEvidence::missing(),
            Some(texts) => CandidateEvidence {
                status: RecognitionEvidenceStatus::Observed,
                hypotheses: texts
                    .into_iter()
                    .enumerate()
                    .map(|(i, text)| RecognitionCandidate {
                        rank: i + 1,
                        text,
                        scores: vec![],
                        alignment: vec![],
                    })
                    .collect(),
                truncated: Some(true),
                origin: Some("synthetic-contract-nbest".into()),
                reason: None,
            },
        };
        candidates.align(&case.text)?;
        let evidence = RecognizerEvidence {
            schema_version: RECOGNIZER_EVIDENCE_SCHEMA.into(),
            source: case.source,
            recognizer: RecognizerIdentity {
                engine: "synthetic-contract".into(),
                model: None,
                version: None,
                decoder: None,
            },
            profile: RecognitionSourceProfile::new("synthetic.candidates.v1", case.source),
            confidences: vec![],
            candidates,
            anchors: vec![],
        };
        let mut input =
            RecognitionInput::new(&case.text, case.source, "synthetic-contract", &case.id);
        input.recognizer_evidence = Some(&evidence);
        let before = baseline.evaluate_recognition(input.clone())?;
        let after = candidate_review.evaluate_recognition(input)?;
        if before.decision != case.baseline
            || after.decision != case.candidate_review
            || before.metrics.slm_calls != 0
            || after.metrics.slm_calls != 0
            || before.recognition_risk.value.is_some()
            || after.recognition_risk.value.is_some()
        {
            return Err(format!("contract failed: {}", case.id).into());
        }
        output.push(serde_json::json!({"id":case.id,"baseline":before,"candidate_review":after}));
    }
    if output.is_empty() {
        return Err("empty contract fixture".into());
    }
    let summary = serde_json::json!({"case_count":output.len(),"quality_accepted":false,
        "dataset_role":"synthetic_operational_contract_not_quality_test","slm_calls":0,
        "baseline_review_count":output.iter().filter(|v|v["baseline"]["decision"]=="review").count(),
        "candidate_review_count":output.iter().filter(|v|v["candidate_review"]["decision"]=="review").count(),
        "input_sha256":format!("{:x}",Sha256::digest(bytes))});
    let result = serde_json::json!({"schema_version":"kzn.candidate_review.observation.v1","summary":summary,"reports":output});
    let path = Path::new(&args[2]);
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_vec_pretty(&result)?)?;
    println!("{summary}");
    Ok(())
}
