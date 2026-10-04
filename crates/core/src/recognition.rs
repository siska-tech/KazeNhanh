//! Recognition risk contract. Screening and typed source evidence, no risk estimator yet.
use crate::*;

pub const RECOGNITION_SCHEMA_VERSION: &str = "kzn.recognition.v2";

wire_enum!(RecognitionSource { Ocr, Asr });
wire_enum!(RecognitionDecision {
    LowRisk,
    Review,
    Undetermined
});
wire_enum!(EvidenceAdequacy {
    Sufficient,
    Limited,
    Insufficient
});
wire_enum!(RecognitionAssessmentStatus {
    Estimated,
    InsufficientEvidence,
    OutOfDomain,
    Failed,
    NotApplicable
});
wire_enum!(RecognitionRiskTarget {
    SegmentContainsTranscriptionError
});
wire_enum!(RecognitionEvidenceKind {
    Morphology,
    TextRules,
    Recognizer,
    LexicalStatistics,
    LanguageModel,
    RiskEstimator
});
wire_enum!(RecognitionEvidenceStatus {
    Observed,
    Missing,
    Unsupported,
    Invalid,
    Failed,
    BudgetSkipped
});
wire_enum!(RecognitionFindingKind {
    Anomaly,
    InputConstraint
});

mod source_evidence;
pub use source_evidence::*;

impl RecognitionSource {
    fn source_kind(self) -> SourceKind {
        match self {
            Self::Ocr => SourceKind::Ocr,
            Self::Asr => SourceKind::Asr,
        }
    }
}

/// Raw source annotations are retained; an arbitrary JSON confidence is not scored.
#[derive(Clone, Debug)]
pub struct RecognitionInput<'a> {
    pub text: &'a str,
    pub language: &'a str,
    pub source: RecognitionSource,
    pub document_id: &'a str,
    pub segment_id: &'a str,
    pub domain: Option<&'a str>,
    pub annotations: &'a [SourceAnnotation],
    pub recognizer_evidence: Option<&'a RecognizerEvidence>,
}
impl<'a> RecognitionInput<'a> {
    pub fn new(
        text: &'a str,
        source: RecognitionSource,
        document_id: &'a str,
        segment_id: &'a str,
    ) -> Self {
        Self {
            text,
            language: "ja",
            source,
            document_id,
            segment_id,
            domain: None,
            annotations: &[],
            recognizer_evidence: None,
        }
    }
}

/// Probability of the declared transcription-error target, high means more risk.
/// Heuristic anomaly indices belong in evidence, not in this field.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecognitionRisk {
    pub target: RecognitionRiskTarget,
    pub value: Option<UnitScore>,
    pub status: RecognitionAssessmentStatus,
    pub method: Option<ScoreMethod>,
    pub calibration_id: Option<String>,
}
impl RecognitionRisk {
    fn validate(&self) -> Result<(), EvaluationError> {
        let estimated = self.status == RecognitionAssessmentStatus::Estimated;
        if estimated != self.value.is_some()
            || estimated != self.calibration_id.is_some()
            || self.method
                != if estimated {
                    Some(ScoreMethod::Calibrated)
                } else {
                    None
                }
            || self
                .calibration_id
                .as_ref()
                .is_some_and(|id| id.trim().is_empty())
        {
            return Err(EvaluationError::Contract(
                "recognition probabilities require an estimated status and calibration identity"
                    .into(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecognitionEvidence {
    pub kind: RecognitionEvidenceKind,
    pub status: RecognitionEvidenceStatus,
    pub span: ByteSpan,
    pub provider_id: String,
    pub value: Option<serde_json::Value>,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecognitionFinding {
    pub kind: RecognitionFindingKind,
    pub issue: Issue,
}

/// Processing the whole string does not establish coverage of all recognition errors.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecognitionCoverage {
    pub processed_spans: Vec<ByteSpan>,
    pub risk_assessed_spans: Vec<ByteSpan>,
    pub risk_unassessed_spans: Vec<ByteSpan>,
    pub source_completeness_assessed: bool,
}

/// R0 has no accepted low-risk policy. A future policy must identify its calibration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecognitionDecisionPolicy {
    pub id: String,
    pub low_risk_threshold: Option<UnitScore>,
    pub calibration_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecognitionReport {
    pub schema_version: String,
    pub original_text: String,
    pub language: String,
    pub source: RecognitionSource,
    pub document_id: String,
    pub segment_id: String,
    pub domain: Option<String>,
    pub annotations: Vec<SourceAnnotation>,
    pub recognizer_evidence: Option<RecognizerEvidence>,
    pub profile_id: String,
    pub transcription_policy_id: Option<String>,
    pub recognition_risk: RecognitionRisk,
    pub evidence_adequacy: EvidenceAdequacy,
    pub decision: RecognitionDecision,
    pub decision_policy: RecognitionDecisionPolicy,
    pub reasons: Vec<String>,
    pub evidence: Vec<RecognitionEvidence>,
    pub findings: Vec<RecognitionFinding>,
    pub coverage: RecognitionCoverage,
    /// Naturalness remains unassessed by the R0 recognition path.
    pub naturalness: DimensionScore,
    pub metrics: EvaluationMetrics,
    pub provenance: Vec<ArtifactIdentity>,
    pub limitations: Vec<String>,
}
impl RecognitionReport {
    /// Validate after deserializing. This verifies structural consistency, not model quality.
    pub fn validate(&self) -> Result<(), EvaluationError> {
        if self.schema_version != RECOGNITION_SCHEMA_VERSION
            || self.language != "ja"
            || self.document_id.trim().is_empty()
            || self.segment_id.trim().is_empty()
            || self.profile_id.trim().is_empty()
            || self.decision_policy.id.trim().is_empty()
            || self.reasons.is_empty()
            || self.reasons.iter().any(|r| r.trim().is_empty())
        {
            return Err(EvaluationError::Contract(
                "invalid recognition report identity/reasons".into(),
            ));
        }
        if let Some(source_evidence) = &self.recognizer_evidence {
            source_evidence.validate(&self.original_text, self.source)?;
            if self.profile_id != source_evidence.profile.id
                || self.transcription_policy_id != source_evidence.profile.transcription_policy_id
            {
                return Err(EvaluationError::Contract(
                    "recognizer profile must match report profile".into(),
                ));
            }
            if !source_evidence.missing_required_signals().is_empty()
                && (self.evidence_adequacy == EvidenceAdequacy::Sufficient
                    || self.decision == RecognitionDecision::LowRisk
                    || !self
                        .reasons
                        .iter()
                        .any(|reason| reason == "required_source_signals_missing"))
            {
                return Err(EvaluationError::Contract(
                    "missing required source signals cannot establish sufficient evidence".into(),
                ));
            }
            let row = self
                .evidence
                .iter()
                .find(|e| e.kind == RecognitionEvidenceKind::Recognizer);
            if row.is_none_or(|e| {
                e.status != RecognitionEvidenceStatus::Observed
                    || e.value.as_ref() != Some(&source_evidence.summary())
            }) {
                return Err(EvaluationError::Contract(
                    "recognizer summary must match typed evidence".into(),
                ));
            }
        } else if self.evidence.iter().any(|e| {
            e.kind == RecognitionEvidenceKind::Recognizer
                && e.status == RecognitionEvidenceStatus::Observed
        }) {
            return Err(EvaluationError::Contract(
                "observed recognizer evidence needs typed source data".into(),
            ));
        }
        self.recognition_risk.validate()?;
        if self
            .transcription_policy_id
            .as_ref()
            .is_some_and(|id| id.trim().is_empty())
            || self.recognition_risk.status == RecognitionAssessmentStatus::Estimated
                && self.transcription_policy_id.is_none()
        {
            return Err(EvaluationError::Contract(
                "an estimated recognition risk requires a transcription policy".into(),
            ));
        }
        self.naturalness.validate()?;
        if self.naturalness.scope != ScoreScope::FullText {
            return Err(EvaluationError::Contract(
                "recognition naturalness must describe the supplied text".into(),
            ));
        }
        for annotation in &self.annotations {
            annotation.span.validate(&self.original_text)?;
        }
        let kinds = [
            RecognitionEvidenceKind::Morphology,
            RecognitionEvidenceKind::TextRules,
            RecognitionEvidenceKind::Recognizer,
            RecognitionEvidenceKind::LexicalStatistics,
            RecognitionEvidenceKind::LanguageModel,
            RecognitionEvidenceKind::RiskEstimator,
        ];
        if self.evidence.len() != kinds.len() {
            return Err(EvaluationError::Contract(
                "each recognition evidence family needs an explicit state".into(),
            ));
        }
        for kind in kinds {
            if self.evidence.iter().filter(|e| e.kind == kind).count() != 1 {
                return Err(EvaluationError::Contract(
                    "duplicate/missing recognition evidence family".into(),
                ));
            }
        }
        for evidence in &self.evidence {
            evidence.span.validate(&self.original_text)?;
            if evidence.provider_id.trim().is_empty()
                || evidence.reason.trim().is_empty()
                || (evidence.status == RecognitionEvidenceStatus::Observed)
                    != evidence
                        .value
                        .as_ref()
                        .is_some_and(|value| !value.is_null())
                || evidence.status != RecognitionEvidenceStatus::Observed
                    && evidence.value.is_some()
            {
                return Err(EvaluationError::Contract(
                    "inconsistent recognition evidence state/value".into(),
                ));
            }
        }
        if self.coverage.source_completeness_assessed {
            return Err(EvaluationError::Contract(
                "source completeness is not supported by the current recognition contract".into(),
            ));
        }
        let whole = vec![ByteSpan::whole(&self.original_text)];
        let estimated = self.recognition_risk.status == RecognitionAssessmentStatus::Estimated;
        let estimator_observed = self.evidence.iter().any(|e| {
            e.kind == RecognitionEvidenceKind::RiskEstimator
                && e.status == RecognitionEvidenceStatus::Observed
        });
        if self.coverage.processed_spans != whole
            || self.coverage.risk_assessed_spans != if estimated { whole.clone() } else { vec![] }
            || self.coverage.risk_unassessed_spans != if estimated { vec![] } else { whole }
            || estimated != estimator_observed
            || self.evidence_adequacy == EvidenceAdequacy::Sufficient && !estimated
        {
            return Err(EvaluationError::Contract(
                "processing coverage cannot substitute for risk assessment".into(),
            ));
        }
        for finding in &self.findings {
            finding.issue.span.validate(&self.original_text)?;
            if finding.issue.code.trim().is_empty() || finding.issue.explanation.trim().is_empty() {
                return Err(EvaluationError::Contract(
                    "invalid recognition finding".into(),
                ));
            }
        }
        let needs_review = self
            .findings
            .iter()
            .any(|f| matches!(f.issue.severity, Severity::Warning | Severity::Error));
        if needs_review && self.decision != RecognitionDecision::Review {
            return Err(EvaluationError::Contract(
                "unresolved findings require review".into(),
            ));
        }
        let policy = &self.decision_policy;
        if policy.low_risk_threshold.is_some() != policy.calibration_id.is_some()
            || policy
                .calibration_id
                .as_ref()
                .is_some_and(|id| id.trim().is_empty())
        {
            return Err(EvaluationError::Contract(
                "low-risk policy requires calibration identity".into(),
            ));
        }
        if self.decision == RecognitionDecision::LowRisk {
            let within_threshold = self
                .recognition_risk
                .value
                .zip(policy.low_risk_threshold)
                .is_some_and(|(risk, max)| risk.value() <= max.value());
            if !estimated
                || self.evidence_adequacy != EvidenceAdequacy::Sufficient
                || !within_threshold
                || self.recognition_risk.calibration_id != policy.calibration_id
                || needs_review
            {
                return Err(EvaluationError::Contract("low-risk decision requires sufficient calibrated evidence and an applicable policy".into()));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct RecognitionConfig {
    pub max_input_bytes: usize,
}
impl Default for RecognitionConfig {
    fn default() -> Self {
        Self {
            max_input_bytes: EvaluationConfig::default().max_input_bytes,
        }
    }
}

/// Model-free R0 path. Old screening verdicts/scores are never promoted to low risk.
/// No secondary worker can be attached to this engine.
pub struct RecognitionEngine {
    screening: EvaluationEngine,
}
impl RecognitionEngine {
    pub fn new(
        analyzer: Arc<dyn MorphAnalyzer>,
        config: RecognitionConfig,
    ) -> Result<Self, EvaluationError> {
        let profile = DomainProfile::screening();
        let mut evaluation = profile.evaluation_config();
        evaluation.max_input_bytes = config.max_input_bytes;
        Ok(Self {
            screening: EvaluationEngine::new(analyzer, evaluation)?
                .with_primary_detector(Arc::new(PrimaryRules::new(profile)?)),
        })
    }

    pub fn evaluate_recognition(
        &self,
        input: RecognitionInput<'_>,
    ) -> Result<RecognitionReport, EvaluationError> {
        if input.document_id.trim().is_empty() || input.segment_id.trim().is_empty() {
            return Err(EvaluationError::InvalidInput(
                "document and segment identities are required".into(),
            ));
        }
        if input.language != "ja" || input.text.len() > self.screening.config.max_input_bytes {
            return Err(EvaluationError::InvalidInput(
                "only ja is supported; input must fit max_input_bytes".into(),
            ));
        }
        if let Some(evidence) = input.recognizer_evidence {
            evidence.validate(input.text, input.source)?;
        }
        let mut text_input = TextInput::new(input.text);
        text_input.language = input.language;
        text_input.source = input.source.source_kind();
        text_input.domain = input.domain;
        text_input.annotations = input.annotations;
        let screening = self.screening.evaluate(text_input)?;
        let whole = ByteSpan::whole(input.text);
        let observed = |kind, provider: &str, value, reason: &str| RecognitionEvidence {
            kind,
            status: RecognitionEvidenceStatus::Observed,
            span: whole,
            provider_id: provider.into(),
            value: Some(value),
            reason: reason.into(),
        };
        let unavailable = |kind, status, reason: &str| RecognitionEvidence {
            kind,
            status,
            span: whole,
            provider_id: "kzn.recognition.r0.v1".into(),
            value: None,
            reason: reason.into(),
        };
        let findings: Vec<_> = screening
            .issues
            .into_iter()
            .map(|issue| RecognitionFinding {
                kind: if issue.severity == Severity::Error {
                    RecognitionFindingKind::InputConstraint
                } else {
                    RecognitionFindingKind::Anomaly
                },
                issue,
            })
            .collect();
        let review = findings
            .iter()
            .any(|f| matches!(f.issue.severity, Severity::Warning | Severity::Error));
        let mut reasons = vec![
            "recognition_risk_estimator_not_configured".into(),
            "evidence_does_not_establish_low_risk".into(),
        ];
        if review {
            reasons.push("primary_findings_require_review".into());
        }
        let mut report = RecognitionReport {
            schema_version: RECOGNITION_SCHEMA_VERSION.into(),
            original_text: input.text.into(),
            language: input.language.into(),
            source: input.source,
            document_id: input.document_id.into(),
            segment_id: input.segment_id.into(),
            domain: input.domain.map(str::to_owned),
            annotations: screening.annotations,
            recognizer_evidence: input.recognizer_evidence.cloned(),
            profile_id: "ja.recognition.r0.v1".into(),
            transcription_policy_id: None,
            recognition_risk: RecognitionRisk {
                target: RecognitionRiskTarget::SegmentContainsTranscriptionError,
                value: None,
                status: RecognitionAssessmentStatus::InsufficientEvidence,
                method: None,
                calibration_id: None,
            },
            evidence_adequacy: EvidenceAdequacy::Limited,
            decision: if review {
                RecognitionDecision::Review
            } else {
                RecognitionDecision::Undetermined
            },
            decision_policy: RecognitionDecisionPolicy {
                id: "kzn.recognition.abstain.v1".into(),
                low_risk_threshold: None,
                calibration_id: None,
            },
            reasons,
            evidence: vec![
                observed(
                    RecognitionEvidenceKind::Morphology,
                    FEATURE_VERSION,
                    serde_json::to_value(&screening.metrics.morphology)
                        .expect("finite morphology counts"),
                    "morphology_features_are_not_recognition_correctness",
                ),
                observed(
                    RecognitionEvidenceKind::TextRules,
                    PRIMARY_RULES_ID,
                    serde_json::json!({"finding_count":findings.len()}),
                    "limited_text_rules_only",
                ),
                unavailable(
                    RecognitionEvidenceKind::Recognizer,
                    if input.annotations.is_empty() {
                        RecognitionEvidenceStatus::Missing
                    } else {
                        RecognitionEvidenceStatus::Unsupported
                    },
                    if input.annotations.is_empty() {
                        "recognizer_evidence_not_provided"
                    } else {
                        "raw_annotations_preserved_not_interpreted"
                    },
                ),
                unavailable(
                    RecognitionEvidenceKind::LexicalStatistics,
                    RecognitionEvidenceStatus::Unsupported,
                    "lexical_statistics_not_implemented",
                ),
                unavailable(
                    RecognitionEvidenceKind::LanguageModel,
                    RecognitionEvidenceStatus::Unsupported,
                    "language_model_not_configured",
                ),
                unavailable(
                    RecognitionEvidenceKind::RiskEstimator,
                    RecognitionEvidenceStatus::Unsupported,
                    "recognition_risk_estimator_not_configured",
                ),
            ],
            findings,
            coverage: RecognitionCoverage {
                processed_spans: vec![whole],
                risk_assessed_spans: vec![],
                risk_unassessed_spans: vec![whole],
                source_completeness_assessed: false,
            },
            naturalness: DimensionScore::unassessed(
                ScoreScope::FullText,
                ScoreStatus::NotEvaluated,
            ),
            metrics: screening.metrics,
            provenance: screening.provenance,
            limitations: vec![
                "r0_screening_evidence_only".into(),
                "transcription_policy_not_configured".into(),
                "no_accepted_low_risk_policy".into(),
                "source_specific_evidence_not_interpreted".into(),
                "only_supplied_text_processed_source_completeness_unknown".into(),
                "fluent_recognition_errors_may_be_unobservable".into(),
            ],
        };
        report.provenance.push(ArtifactIdentity {
            component: "recognition_policy".into(),
            id: report.decision_policy.id.clone(),
            sha256: None,
        });
        if let Some(source_evidence) = input.recognizer_evidence {
            report.profile_id = source_evidence.profile.id.clone();
            if !source_evidence.missing_required_signals().is_empty() {
                report
                    .reasons
                    .push("required_source_signals_missing".into());
            }
            report.transcription_policy_id =
                source_evidence.profile.transcription_policy_id.clone();
            let row = report
                .evidence
                .iter_mut()
                .find(|e| e.kind == RecognitionEvidenceKind::Recognizer)
                .expect("fixed evidence families");
            row.status = RecognitionEvidenceStatus::Observed;
            row.provider_id = "kzn.source_evidence.v1".into();
            row.value = Some(source_evidence.summary());
            row.reason = "typed_recognizer_evidence_not_correctness_probability".into();
            report.limitations.retain(|reason| {
                reason != "source_specific_evidence_not_interpreted"
                    && !(reason == "transcription_policy_not_configured"
                        && report.transcription_policy_id.is_some())
            });
            report
                .limitations
                .push("source_scores_not_normalized_or_fused".into());
            report.provenance.push(ArtifactIdentity {
                component: "source_profile".into(),
                id: source_evidence.profile.id.clone(),
                sha256: None,
            });
        }
        report.validate()?;
        Ok(report)
    }
    pub fn evaluate_recognition_batch(
        &self,
        inputs: &[RecognitionInput<'_>],
    ) -> Vec<Result<RecognitionReport, EvaluationError>> {
        inputs
            .iter()
            .map(|input| self.evaluate_recognition(input.clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests;
