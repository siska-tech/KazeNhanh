//! Backend-independent detection contracts. Scores never imply world factuality.
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;

mod secondary;
pub use secondary::{
    SecondaryEvaluation, SecondaryFailure, SecondaryJudge, SecondaryJudgeFactory, SecondaryPolicy,
    SecondaryRequest, SecondaryWorker,
};
mod primary;
pub use primary::{DomainProfile, MorphologyFeatures, PrimaryRules, TextFormat, PRIMARY_RULES_ID};

pub const SCHEMA_VERSION: &str = "kzn.evaluation.v3";
pub const FEATURE_VERSION: &str = "kzn.morphology.v1";

#[derive(Debug, Error, PartialEq)]
pub enum EvaluationError {
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Invalid UTF-8 byte span [{start}, {end})")]
    InvalidSpan { start: usize, end: usize },
    #[error("Backend {backend} failed: {message}")]
    Backend { backend: String, message: String },
    #[error("Backend contract violation: {0}")]
    Contract(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ByteSpan {
    start: usize,
    end: usize,
}
impl ByteSpan {
    pub fn new(text: &str, start: usize, end: usize) -> Result<Self, EvaluationError> {
        let span = Self { start, end };
        span.validate(text)?;
        Ok(span)
    }
    pub fn whole(text: &str) -> Self {
        Self {
            start: 0,
            end: text.len(),
        }
    }
    pub fn start(self) -> usize {
        self.start
    }
    pub fn end(self) -> usize {
        self.end
    }
    pub fn validate(self, text: &str) -> Result<(), EvaluationError> {
        if self.start > self.end
            || self.end > text.len()
            || !text.is_char_boundary(self.start)
            || !text.is_char_boundary(self.end)
        {
            return Err(EvaluationError::InvalidSpan {
                start: self.start,
                end: self.end,
            });
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(transparent)]
pub struct UnitScore(f32);
impl UnitScore {
    pub fn new(value: f32) -> Result<Self, EvaluationError> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(EvaluationError::Contract(
                "score must be finite and in 0..=1".into(),
            ));
        }
        Ok(Self(value))
    }
    pub fn value(self) -> f32 {
        self.0
    }
}
impl<'de> Deserialize<'de> for UnitScore {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(f32::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    PlainText,
    Ocr,
    Asr,
    Llm,
    Form,
    Other(String),
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceAnnotation {
    pub span: ByteSpan,
    pub data: serde_json::Value,
}
#[derive(Clone, Debug)]
pub struct TextInput<'a> {
    pub text: &'a str,
    pub language: &'a str,
    pub source: SourceKind,
    pub domain: Option<&'a str>,
    pub reference: Option<&'a str>,
    pub annotations: &'a [SourceAnnotation],
}
impl<'a> TextInput<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            language: "ja",
            source: SourceKind::PlainText,
            domain: None,
            reference: None,
            annotations: &[],
        }
    }
}

macro_rules! wire_enum {
    ($name:ident { $($variant:ident),* }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($variant),* }
    }
}
wire_enum!(Verdict {
    Acceptable,
    Suspicious,
    Invalid,
    Undetermined
});
wire_enum!(ScoreStatus {
    Evaluated,
    NotEvaluated,
    NotApplicable,
    InsufficientContext,
    Failed
});
wire_enum!(ScoreMethod {
    Heuristic,
    Model,
    Calibrated
});
wire_enum!(ScoreScope {
    FullText,
    Internal,
    Reference
});
wire_enum!(Dimension {
    Validity,
    Naturalness,
    SemanticConsistency
});
wire_enum!(Severity {
    Info,
    Warning,
    Error
});
wire_enum!(DetectionStage { Primary, Secondary });
wire_enum!(RoutingStatus {
    NotRequested,
    Disabled,
    BudgetExceeded,
    Timeout,
    BackendError,
    Completed,
    ContextMissing,
    InvalidOutput
});

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DimensionScore {
    pub value: Option<UnitScore>,
    pub status: ScoreStatus,
    pub method: Option<ScoreMethod>,
    pub scope: ScoreScope,
    pub calibration_id: Option<String>,
    pub confidence: Option<UnitScore>,
}
impl DimensionScore {
    pub fn unassessed(scope: ScoreScope, status: ScoreStatus) -> Self {
        Self {
            value: None,
            status,
            method: None,
            scope,
            calibration_id: None,
            confidence: None,
        }
    }
    pub fn validate(&self) -> Result<(), EvaluationError> {
        if (self.status == ScoreStatus::Evaluated) != self.value.is_some()
            || (self.status == ScoreStatus::Evaluated) != self.method.is_some()
            || (self.method == Some(ScoreMethod::Calibrated)) != self.calibration_id.is_some()
            || (self.status != ScoreStatus::Evaluated && self.confidence.is_some())
        {
            return Err(EvaluationError::Contract(
                "inconsistent score status/value/method/calibration".into(),
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scores {
    pub validity: DimensionScore,
    pub naturalness: DimensionScore,
    pub semantic_consistency: DimensionScore,
}
impl Scores {
    fn values(&self) -> [&DimensionScore; 3] {
        [
            &self.validity,
            &self.naturalness,
            &self.semantic_consistency,
        ]
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Issue {
    pub code: String,
    pub severity: Severity,
    pub span: ByteSpan,
    pub stage: DetectionStage,
    pub evidence: serde_json::Value,
    pub explanation: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactIdentity {
    pub component: String,
    pub id: String,
    pub sha256: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Morpheme {
    pub span: ByteSpan,
    pub surface: String,
    pub dictionary_form: String,
    pub normalized_form: String,
    pub reading: String,
    pub part_of_speech: Vec<String>,
    pub is_oov: bool,
    pub dictionary_id: i32,
    pub synonym_group_ids: Vec<u32>,
    /// Backend path cost; neither per-word probability nor calibrated confidence.
    pub cumulative_cost: Option<i32>,
}
#[derive(Clone, Debug, Default)]
pub struct MorphAnalysis {
    pub morphemes: Vec<Morpheme>,
    pub provenance: Vec<ArtifactIdentity>,
}
pub trait MorphAnalyzer: Send + Sync {
    fn analyze(&self, text: &str) -> Result<MorphAnalysis, EvaluationError>;
}
#[derive(Clone, Debug)]
pub struct PrimaryEvaluation {
    pub verdict: Verdict,
    pub scores: Scores,
    pub issues: Vec<Issue>,
}
pub trait PrimaryDetector: Send + Sync {
    fn profile(&self) -> Option<DomainProfile> {
        None
    }
    fn artifacts(&self) -> Vec<ArtifactIdentity> {
        vec![]
    }
    fn limitations(&self) -> Vec<String> {
        vec![]
    }
    fn detect(
        &self,
        input: &TextInput<'_>,
        analysis: &MorphAnalysis,
        config: &EvaluationConfig,
    ) -> Result<PrimaryEvaluation, EvaluationError>;
}
#[derive(Clone, Debug)]
pub struct EvaluationConfig {
    pub profile_id: String,
    pub max_input_bytes: usize,
    pub semantic_scope: ScoreScope,
    /// Missing mandatory dimensions can never produce acceptable.
    pub required_dimensions: Vec<Dimension>,
}
impl Default for EvaluationConfig {
    fn default() -> Self {
        Self {
            profile_id: "ja.primary.v1".into(),
            max_input_bytes: 1_048_576,
            semantic_scope: ScoreScope::Internal,
            required_dimensions: vec![Dimension::Validity, Dimension::Naturalness],
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Routing {
    pub secondary_needed: Option<bool>,
    pub status: RoutingStatus,
    pub reasons: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DimensionCoverage {
    pub dimension: Dimension,
    pub scope: ScoreScope,
    pub evaluated_spans: Vec<ByteSpan>,
    pub unassessed_spans: Vec<ByteSpan>,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvaluationMetrics {
    pub morpheme_count: usize,
    pub slm_calls: usize,
    pub morphology: MorphologyFeatures,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvaluationReport {
    pub schema_version: String,
    pub feature_version: String,
    pub original_text: String,
    pub language: String,
    pub source: SourceKind,
    pub domain: Option<String>,
    pub annotations: Vec<SourceAnnotation>,
    pub reference_provided: bool,
    pub profile_id: String,
    pub profile_config: Option<DomainProfile>,
    pub required_dimensions: Vec<Dimension>,
    pub verdict: Verdict,
    pub scores: Scores,
    pub issues: Vec<Issue>,
    pub routing: Routing,
    pub coverage: Vec<DimensionCoverage>,
    pub limitations: Vec<String>,
    pub provenance: Vec<ArtifactIdentity>,
    pub metrics: EvaluationMetrics,
}
impl EvaluationReport {
    /// Validate after deserialization too: span boundaries require original text.
    pub fn validate(&self) -> Result<(), EvaluationError> {
        if self.schema_version != SCHEMA_VERSION || self.feature_version != FEATURE_VERSION {
            return Err(EvaluationError::Contract(
                "unsupported schema/feature version".into(),
            ));
        }
        if let Some(profile) = &self.profile_config {
            profile.validate()?;
            if profile.id != self.profile_id {
                return Err(EvaluationError::Contract(
                    "profile metadata mismatch".into(),
                ));
            }
        }
        for score in self.scores.values() {
            score.validate()?;
        }
        if self.verdict == Verdict::Acceptable
            && self.required_dimensions.iter().any(|dimension| {
                let score = match dimension {
                    Dimension::Validity => &self.scores.validity,
                    Dimension::Naturalness => &self.scores.naturalness,
                    Dimension::SemanticConsistency => &self.scores.semantic_consistency,
                };
                score.status != ScoreStatus::Evaluated
            })
        {
            return Err(EvaluationError::Contract(
                "acceptable requires all mandatory dimensions".into(),
            ));
        }
        if self.verdict == Verdict::Acceptable
            && self
                .issues
                .iter()
                .any(|issue| issue.severity != Severity::Info)
        {
            return Err(EvaluationError::Contract(
                "acceptable cannot contain unresolved warning/error findings".into(),
            ));
        }
        if self.metrics.slm_calls > 1
            || (matches!(
                self.routing.status,
                RoutingStatus::Disabled
                    | RoutingStatus::NotRequested
                    | RoutingStatus::ContextMissing
            ) && self.metrics.slm_calls != 0)
            || (self.routing.status == RoutingStatus::Completed && self.metrics.slm_calls != 1)
            || (self.routing.status != RoutingStatus::NotRequested
                && self.routing.secondary_needed != Some(true))
            || (matches!(
                self.routing.status,
                RoutingStatus::BudgetExceeded
                    | RoutingStatus::Timeout
                    | RoutingStatus::BackendError
                    | RoutingStatus::InvalidOutput
                    | RoutingStatus::ContextMissing
            ) && self.verdict == Verdict::Acceptable)
            || (!self.reference_provided
                && self.scores.semantic_consistency.scope == ScoreScope::Reference
                && self.scores.semantic_consistency.status == ScoreStatus::Evaluated)
        {
            return Err(EvaluationError::Contract(
                "inconsistent secondary routing/call/context metadata".into(),
            ));
        }
        if self.metrics.morpheme_count != self.metrics.morphology.token_count {
            return Err(EvaluationError::Contract(
                "morphology token count mismatch".into(),
            ));
        }
        if self.coverage.len() != 3 {
            return Err(EvaluationError::Contract(
                "coverage must describe all three dimensions".into(),
            ));
        }
        for (dimension, score) in [
            Dimension::Validity,
            Dimension::Naturalness,
            Dimension::SemanticConsistency,
        ]
        .into_iter()
        .zip(self.scores.values())
        {
            let rows = self
                .coverage
                .iter()
                .filter(|row| row.dimension == dimension)
                .collect::<Vec<_>>();
            if rows.len() != 1 || rows[0].scope != score.scope {
                return Err(EvaluationError::Contract(
                    "coverage dimension/scope mismatch".into(),
                ));
            }
            let row = rows[0];
            let whole = vec![ByteSpan::whole(&self.original_text)];
            let evaluated = score.status == ScoreStatus::Evaluated;
            if (evaluated && (row.evaluated_spans != whole || !row.unassessed_spans.is_empty()))
                || (!evaluated
                    && (row.unassessed_spans != whole || !row.evaluated_spans.is_empty()))
            {
                return Err(EvaluationError::Contract(
                    "primary screening requires full-input assessed or unassessed coverage".into(),
                ));
            }
        }
        for annotation in &self.annotations {
            annotation.span.validate(&self.original_text)?;
        }
        for issue in &self.issues {
            issue.span.validate(&self.original_text)?;
        }
        for coverage in &self.coverage {
            for span in coverage
                .evaluated_spans
                .iter()
                .chain(&coverage.unassessed_spans)
            {
                span.validate(&self.original_text)?;
            }
        }
        Ok(())
    }
}

pub struct EvaluationEngine {
    analyzer: Arc<dyn MorphAnalyzer>,
    detector: Option<Arc<dyn PrimaryDetector>>,
    config: EvaluationConfig,
    secondary: Option<Arc<SecondaryWorker>>,
}
impl EvaluationEngine {
    pub fn new(
        analyzer: Arc<dyn MorphAnalyzer>,
        mut config: EvaluationConfig,
    ) -> Result<Self, EvaluationError> {
        if config.max_input_bytes == 0
            || config.profile_id.trim().is_empty()
            || config.semantic_scope == ScoreScope::FullText
        {
            return Err(EvaluationError::InvalidConfig("nonempty profile, positive byte limit and internal/reference semantic scope required".into()));
        }
        if config.semantic_scope == ScoreScope::Reference
            && !config
                .required_dimensions
                .contains(&Dimension::SemanticConsistency)
        {
            config
                .required_dimensions
                .push(Dimension::SemanticConsistency);
        }
        Ok(Self {
            analyzer,
            detector: None,
            secondary: None,
            config,
        })
    }
    pub fn with_primary_detector(mut self, detector: Arc<dyn PrimaryDetector>) -> Self {
        self.detector = Some(detector);
        self
    }
    pub fn evaluate(&self, input: TextInput<'_>) -> Result<EvaluationReport, EvaluationError> {
        if input.language != "ja" || input.text.len() > self.config.max_input_bytes {
            return Err(EvaluationError::InvalidInput(
                "only ja is supported; input must fit max_input_bytes".into(),
            ));
        }
        for annotation in input.annotations {
            annotation.span.validate(input.text)?;
        }
        let analysis = self.analyzer.analyze(input.text)?;
        let mut previous_end = 0;
        for token in &analysis.morphemes {
            token.span.validate(input.text)?;
            if token.span.start() < previous_end
                || token.span.start() == token.span.end()
                || &input.text[token.span.start()..token.span.end()] != token.surface
            {
                return Err(EvaluationError::Contract(
                    "morphemes must preserve ordered, nonoverlapping raw surfaces".into(),
                ));
            }
            previous_end = token.span.end();
        }
        let mut primary = match &self.detector {
            Some(detector) => detector.detect(&input, &analysis, &self.config)?,
            None => PrimaryEvaluation {
                verdict: Verdict::Undetermined,
                issues: vec![],
                scores: Scores {
                    validity: DimensionScore::unassessed(
                        ScoreScope::FullText,
                        ScoreStatus::NotEvaluated,
                    ),
                    naturalness: DimensionScore::unassessed(
                        ScoreScope::FullText,
                        ScoreStatus::NotEvaluated,
                    ),
                    semantic_consistency: DimensionScore::unassessed(
                        self.config.semantic_scope,
                        ScoreStatus::NotEvaluated,
                    ),
                },
            },
        };
        if primary.scores.validity.scope != ScoreScope::FullText
            || primary.scores.naturalness.scope != ScoreScope::FullText
            || primary.scores.semantic_consistency.scope != self.config.semantic_scope
        {
            return Err(EvaluationError::Contract(
                "detector returned an unexpected score scope".into(),
            ));
        }
        let mut limitations = self
            .detector
            .as_ref()
            .map(|detector| detector.limitations())
            .unwrap_or_default();
        if self.detector.is_none() {
            limitations.push("primary_detector_not_configured".into());
        }
        if self.config.semantic_scope == ScoreScope::Reference
            && input.reference.is_none_or(|r| r.trim().is_empty())
        {
            primary.scores.semantic_consistency =
                DimensionScore::unassessed(ScoreScope::Reference, ScoreStatus::InsufficientContext);
            limitations.push("reference_context_missing".into());
        }
        let scores = primary.scores.values();
        if primary.verdict == Verdict::Acceptable
            && self.config.required_dimensions.iter().any(|dimension| {
                let score = match dimension {
                    Dimension::Validity => scores[0],
                    Dimension::Naturalness => scores[1],
                    Dimension::SemanticConsistency => scores[2],
                };
                score.status != ScoreStatus::Evaluated
            })
        {
            primary.verdict = Verdict::Undetermined;
        }
        let mut routing_reasons = Vec::new();
        let secondary_needed = if self.detector.is_none() {
            None
        } else if primary.verdict == Verdict::Invalid {
            Some(false)
        } else {
            if primary.verdict == Verdict::Suspicious {
                routing_reasons.push("primary_warning_requires_review".into());
            }
            if self.config.required_dimensions.iter().any(|dimension| {
                let score = match dimension {
                    Dimension::Validity => &primary.scores.validity,
                    Dimension::Naturalness => &primary.scores.naturalness,
                    Dimension::SemanticConsistency => &primary.scores.semantic_consistency,
                };
                matches!(
                    score.status,
                    ScoreStatus::NotEvaluated
                        | ScoreStatus::InsufficientContext
                        | ScoreStatus::Failed
                )
            }) {
                routing_reasons.push("required_dimension_unresolved".into());
            }
            if input
                .reference
                .is_some_and(|reference| !reference.trim().is_empty())
                && primary.scores.semantic_consistency.status != ScoreStatus::Evaluated
            {
                routing_reasons.push("reference_consistency_unresolved".into());
                if primary.verdict == Verdict::Acceptable {
                    primary.verdict = Verdict::Undetermined;
                }
            }
            Some(!routing_reasons.is_empty())
        };
        if secondary_needed == Some(true) {
            limitations.push("secondary_judge_not_configured".into());
        }
        if secondary_needed.is_none() {
            routing_reasons.push("primary_detector_not_configured".into());
        }
        let mut provenance = analysis.provenance.clone();
        if let Some(detector) = &self.detector {
            provenance.extend(detector.artifacts());
        }
        let coverage = [
            Dimension::Validity,
            Dimension::Naturalness,
            Dimension::SemanticConsistency,
        ]
        .into_iter()
        .zip(primary.scores.values())
        .map(|(dimension, score)| {
            let evaluated = score.status == ScoreStatus::Evaluated;
            DimensionCoverage {
                dimension,
                scope: score.scope,
                evaluated_spans: if evaluated {
                    vec![ByteSpan::whole(input.text)]
                } else {
                    vec![]
                },
                unassessed_spans: if evaluated {
                    vec![]
                } else {
                    vec![ByteSpan::whole(input.text)]
                },
            }
        })
        .collect();
        let mut report = EvaluationReport {
            schema_version: SCHEMA_VERSION.into(),
            feature_version: FEATURE_VERSION.into(),
            original_text: input.text.into(),
            language: input.language.into(),
            source: input.source.clone(),
            domain: input.domain.map(str::to_owned),
            annotations: input.annotations.to_vec(),
            reference_provided: input.reference.is_some(),
            profile_id: self.config.profile_id.clone(),
            profile_config: self
                .detector
                .as_ref()
                .and_then(|detector| detector.profile()),
            required_dimensions: self.config.required_dimensions.clone(),
            verdict: primary.verdict,
            scores: primary.scores,
            issues: primary.issues,
            routing: Routing {
                secondary_needed,
                status: if secondary_needed == Some(true) {
                    RoutingStatus::Disabled
                } else {
                    RoutingStatus::NotRequested
                },
                reasons: routing_reasons,
            },
            coverage,
            limitations,
            provenance,
            metrics: EvaluationMetrics {
                morpheme_count: analysis.morphemes.len(),
                slm_calls: 0,
                morphology: MorphologyFeatures::extract(&analysis),
            },
        };
        report.validate()?;
        self.apply_secondary(&input, &mut report);
        report.validate()?;
        Ok(report)
    }
    pub fn evaluate_batch(
        &self,
        inputs: &[TextInput<'_>],
    ) -> Vec<Result<EvaluationReport, EvaluationError>> {
        inputs
            .iter()
            .cloned()
            .map(|input| self.evaluate(input))
            .collect()
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod primary_tests;

#[cfg(test)]
mod secondary_tests;
