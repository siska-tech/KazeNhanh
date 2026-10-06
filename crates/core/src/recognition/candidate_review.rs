//! Opt-in operational review baseline. Candidate differences are uncertainty, not error labels.
use super::*;

pub const CANDIDATE_REVIEW_POLICY_ID: &str = "kzn.recognition.raw_candidate_review.v1";
const FINDING_CODE: &str = "candidate_disagreement";
const REASON: &str = "candidate_disagreement_requires_review";

pub(super) fn candidate_findings(source: Option<&RecognizerEvidence>) -> Vec<RecognitionFinding> {
    let Some(source) = source else { return vec![] };
    if source.candidates.status != RecognitionEvidenceStatus::Observed {
        return vec![];
    }
    let Some(first) = source.candidates.hypotheses.first() else {
        return vec![];
    };
    source.candidates.hypotheses.iter().skip(1).filter(|c| c.text != first.text).map(|candidate| {
        // Source validation already established ordered, complete raw coordinates.
        let mut differences = candidate.alignment.iter().filter(|a| a.edit != CandidateEditKind::Equal);
        let first_difference = differences.next().expect("validated unequal candidate has differences");
        let mut last = first_difference;
        let mut block_count = 1;
        for difference in differences { last = difference; block_count += 1; }
        RecognitionFinding {
            kind: RecognitionFindingKind::Anomaly,
            issue: Issue {
                code: FINDING_CODE.into(), severity: Severity::Warning, stage: DetectionStage::Primary,
                span: ByteSpan::new(&first.text, first_difference.original.start(), last.original.end())
                    .expect("validated raw alignment"),
                evidence: serde_json::json!({"policy_id": CANDIDATE_REVIEW_POLICY_ID,
                    "candidate_rank":candidate.rank,"difference_block_count":block_count,
                    "scope":"enclosing_original_difference_span","comparison":"raw_text",
                    "scores_used":false,"alternative_correctness":"unknown"}),
                explanation: "A supplied candidate differs from the 1-best. Review uncertainty; neither candidate is established as correct.".into(),
            },
        }
    }).collect()
}

pub(super) fn validate_candidate_review(report: &RecognitionReport) -> Result<(), EvaluationError> {
    let enabled = matches!(
        report.decision_policy.id.as_str(),
        CANDIDATE_REVIEW_POLICY_ID | SPARSE_CANDIDATE_REVIEW_POLICY_ID
    );
    let expected = if enabled {
        candidate_findings(report.recognizer_evidence.as_ref())
    } else {
        vec![]
    };
    let actual: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.issue.code == FINDING_CODE)
        .cloned()
        .collect();
    if actual != expected || report.reasons.iter().any(|r| r == REASON) != !expected.is_empty() {
        return Err(EvaluationError::Contract(
            "candidate findings must match enabled policy and typed evidence".into(),
        ));
    }
    if enabled {
        let review = report
            .findings
            .iter()
            .any(|f| matches!(f.issue.severity, Severity::Warning | Severity::Error));
        if report.recognition_risk.value.is_some()
            || report.decision_policy.low_risk_threshold.is_some()
            || report.decision_policy.calibration_id.is_some()
            || report.decision
                != if review {
                    RecognitionDecision::Review
                } else {
                    RecognitionDecision::Undetermined
                }
        {
            return Err(EvaluationError::Contract(
                "candidate review baseline cannot estimate risk or accept low risk".into(),
            ));
        }
    }
    Ok(())
}

pub(super) fn candidate_reason() -> &'static str {
    REASON
}
