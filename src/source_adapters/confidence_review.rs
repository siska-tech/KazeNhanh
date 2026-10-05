//! Explicit source-bound review requests. No default thresholds or correctness claims.
use super::*;

#[derive(Clone, Debug)]
pub struct ConfidenceReviewRule {
    pub id: String,
    pub recognizer: RecognizerIdentity,
    pub profile: RecognitionSourceProfile,
    pub domain: String,
    pub granularity: ConfidenceGranularity,
    pub aggregation: String,
    /// Value is the strict review boundary; all other fields describe the expected scale.
    pub threshold: RawScore,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ConfidenceReviewObservation {
    /// Unmodified input, including span, score and shared dependencies.
    pub observation: ConfidenceObservation,
    /// None means not applicable, false only means the explicit threshold was not crossed.
    pub review_requested: Option<bool>,
    pub reason: String,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ConfidenceReviewAssessment {
    pub rule_id: String,
    pub review_requested: bool,
    pub observations: Vec<ConfidenceReviewObservation>,
    /// Global binding failure or an empty confidence set, never a normality claim.
    pub unavailable_reason: Option<String>,
}
fn identity(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 256
}
impl ConfidenceReviewRule {
    pub fn validate(&self) -> Result<(), EvaluationError> {
        self.threshold.validate().map_err(|_| {
            EvaluationError::InvalidConfig("invalid confidence threshold scale".into())
        })?;
        if ![
            self.id.as_str(),
            self.domain.as_str(),
            self.aggregation.as_str(),
            self.recognizer.engine.as_str(),
            self.profile.id.as_str(),
        ]
        .into_iter()
        .all(identity)
            || [
                &self.recognizer.model,
                &self.recognizer.version,
                &self.recognizer.decoder,
                &self.profile.transcription_policy_id,
                &self.threshold.target,
            ]
            .into_iter()
            .any(|v| v.as_deref().is_none_or(|v| !identity(v)))
            || self.threshold.meaning == ConfidenceMeaning::Unknown
            || self.threshold.direction == ConfidenceDirection::Unknown
        {
            return Err(EvaluationError::InvalidConfig("confidence rule requires explicit identity, transcription policy, score meaning/direction/target and aggregation".into()));
        }
        Ok(())
    }
    /// Adapter-side assessment only. Callers must not turn false/None into low-risk acceptance.
    pub fn assess(
        &self,
        text: &str,
        source: RecognitionSource,
        domain: Option<&str>,
        evidence: &RecognizerEvidence,
    ) -> Result<ConfidenceReviewAssessment, EvaluationError> {
        self.validate()?;
        evidence.validate(text, source)?;
        let binding_reason = if evidence.recognizer != self.recognizer {
            Some("recognizer_mismatch")
        } else if evidence.profile != self.profile {
            Some("profile_mismatch")
        } else if domain != Some(self.domain.as_str()) {
            Some("domain_missing_or_mismatch")
        } else {
            None
        };
        let mut rows = Vec::with_capacity(evidence.confidences.len());
        for observation in &evidence.confidences {
            let mut reason = binding_reason;
            if reason.is_none() {
                reason = if observation.status != RecognitionEvidenceStatus::Observed {
                    Some("confidence_not_observed")
                } else if observation.granularity != self.granularity {
                    Some("granularity_mismatch")
                } else if observation.aggregation.as_deref() != Some(self.aggregation.as_str()) {
                    Some("aggregation_missing_or_mismatch")
                } else if observation.span.start() == observation.span.end() {
                    Some("empty_confidence_scope")
                } else {
                    None
                };
            }
            let mut review = None;
            if reason.is_none() {
                let score = observation
                    .score
                    .as_ref()
                    .expect("validated observed score");
                let mut descriptor = score.clone();
                descriptor.value = self.threshold.value;
                if descriptor != self.threshold {
                    reason = Some("score_semantics_mismatch");
                } else {
                    review = Some(match score.direction {
                        ConfidenceDirection::HigherIsBetter => score.value < self.threshold.value,
                        ConfidenceDirection::LowerIsBetter => score.value > self.threshold.value,
                        ConfidenceDirection::Unknown => {
                            unreachable!("rule rejected unknown direction")
                        }
                    });
                }
            }
            rows.push(ConfidenceReviewObservation {
                observation: observation.clone(),
                review_requested: review,
                reason: reason
                    .unwrap_or(if review == Some(true) {
                        "threshold_crossed"
                    } else {
                        "threshold_not_crossed"
                    })
                    .into(),
            });
        }
        Ok(ConfidenceReviewAssessment {
            rule_id: self.id.clone(),
            review_requested: rows.iter().any(|r| r.review_requested == Some(true)),
            unavailable_reason: binding_reason
                .or(if rows.is_empty() {
                    Some("confidence_not_provided")
                } else {
                    None
                })
                .map(str::to_owned),
            observations: rows,
        })
    }
}
