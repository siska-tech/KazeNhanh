//! Experimental conjunctive review, not learned fusion or an error probability.
use super::*;
pub const SPARSE_REVIEW_POLICY_ID: &str = "kzn.recognition.sparse_review.v1";
pub const SPARSE_CANDIDATE_REVIEW_POLICY_ID: &str = "kzn.recognition.sparse_candidate_review.v1";
const CODE: &str = "sparse_statistics_conjunction";
const REASON: &str = "sparse_statistics_require_review";
fn enabled(report: &RecognitionReport) -> bool {
    matches!(
        report.decision_policy.id.as_str(),
        SPARSE_REVIEW_POLICY_ID | SPARSE_CANDIDATE_REVIEW_POLICY_ID
    )
}
fn findings(report: &RecognitionReport) -> Result<Vec<RecognitionFinding>, EvaluationError> {
    let Some(row) = report.evidence.iter().find(|e| {
        e.kind == RecognitionEvidenceKind::LexicalStatistics
            && e.status == RecognitionEvidenceStatus::Observed
    }) else {
        return Ok(vec![]);
    };
    let observation: StatisticsObservation = serde_json::from_value(
        row.value
            .clone()
            .ok_or_else(|| EvaluationError::Contract("missing statistics payload".into()))?,
    )
    .map_err(|_| EvaluationError::Contract("invalid statistics payload".into()))?;
    observation.validate(
        &report.original_text,
        report.domain.as_deref(),
        &row.provider_id,
    )?;
    if observation.oov_count == 0
        || observation.character_pairs.unseen_count == 0
        || observation.word_pairs.unseen_count == 0
    {
        return Ok(vec![]);
    }
    Ok(vec![RecognitionFinding { kind: RecognitionFindingKind::Anomaly, issue: Issue {
        code: CODE.into(), severity: Severity::Warning, stage: DetectionStage::Primary,
        span: ByteSpan::whole(&report.original_text),
        evidence: serde_json::json!({"policy_id":report.decision_policy.id,"asset_id":observation.asset_id,
            "oov_count":observation.oov_count,"unseen_character_pair_count":observation.character_pairs.unseen_count,
            "unseen_word_pair_count":observation.word_pairs.unseen_count,"scope":"segment_cooccurrence",
            "independence_assumed":false,"confidence_used":false,"error_probability":null}),
        explanation: "OOV and corpus-unseen character/word pairs co-occur in this segment. Experimental review only; rarity and correlated features do not establish a recognition error.".into(),
    }}])
}
pub(super) fn apply(report: &mut RecognitionReport) -> Result<(), EvaluationError> {
    let extra = findings(report)?;
    if !extra.is_empty() {
        report.findings.extend(extra);
        report.reasons.push(REASON.into());
        report.decision = RecognitionDecision::Review;
    }
    report
        .limitations
        .push("experimental_sparse_review_may_flag_names_or_domain_shift".into());
    Ok(())
}
pub(super) fn validate(report: &RecognitionReport) -> Result<(), EvaluationError> {
    let expected = if enabled(report) {
        findings(report)?
    } else {
        vec![]
    };
    let actual: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.issue.code == CODE)
        .cloned()
        .collect();
    if actual != expected || report.reasons.iter().any(|r| r == REASON) != !expected.is_empty() {
        return Err(EvaluationError::Contract(
            "sparse review findings must match policy and observations".into(),
        ));
    }
    if enabled(report) {
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
                "sparse review cannot estimate probability or accept low risk".into(),
            ));
        }
    }
    Ok(())
}
