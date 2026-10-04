//! Source-independent recognition evidence. Raw scores never imply correctness.
use super::*;

wire_enum!(RecognizerSignal {
    Confidence,
    Candidates,
    SourceAnchors
});

pub const RECOGNIZER_EVIDENCE_SCHEMA: &str = "kzn.recognizer.v1";
pub const MAX_CANDIDATES: usize = 16;
pub const MAX_CANDIDATE_BYTES: usize = 65_536;
pub const MAX_ALIGNMENT_CELLS: usize = 262_144;
pub const MAX_TOTAL_ALIGNMENT_CELLS: usize = 1_048_576;
pub const MAX_SOURCE_OBSERVATIONS: usize = 1024;

wire_enum!(ConfidenceMeaning {
    Posterior,
    LogProbability,
    Logit,
    EngineScore,
    Unknown
});
wire_enum!(ConfidenceDirection {
    HigherIsBetter,
    LowerIsBetter,
    Unknown
});
wire_enum!(ConfidenceGranularity {
    Character,
    Token,
    Word,
    Line,
    Utterance,
    Segment
});
wire_enum!(CandidateEditKind {
    Equal,
    Substitution,
    Insertion,
    Deletion
});

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawScore {
    pub value: f64,
    pub meaning: ConfidenceMeaning,
    pub direction: ConfidenceDirection,
    pub range: Option<[f64; 2]>,
    pub calibration_id: Option<String>,
    pub target: Option<String>,
}
impl RawScore {
    pub fn validate(&self) -> Result<(), EvaluationError> {
        if !optional_identity(&self.target)
            || self.calibration_id.is_some() && self.target.is_none()
            || !self.value.is_finite()
            || self.range.is_some_and(|r| {
                !r[0].is_finite()
                    || !r[1].is_finite()
                    || r[0] >= r[1]
                    || self.value < r[0]
                    || self.value > r[1]
            })
            || self.meaning == ConfidenceMeaning::Posterior && !(0.0..=1.0).contains(&self.value)
            || self.meaning == ConfidenceMeaning::LogProbability && self.value > 0.0
            || matches!(
                self.meaning,
                ConfidenceMeaning::Posterior | ConfidenceMeaning::LogProbability
            ) && self.direction != ConfidenceDirection::HigherIsBetter
            || self
                .calibration_id
                .as_ref()
                .is_some_and(|id| id.trim().is_empty())
            || self.calibration_id.is_some() && self.meaning != ConfidenceMeaning::Posterior
        {
            return Err(EvaluationError::InvalidInput(
                "invalid raw score semantics/range/calibration".into(),
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfidenceObservation {
    pub id: String,
    pub span: ByteSpan,
    pub granularity: ConfidenceGranularity,
    pub status: RecognitionEvidenceStatus,
    pub score: Option<RawScore>,
    pub aggregation: Option<String>,
    pub reason: Option<String>,
    /// Shared decoder/LM dependencies are explicitly retained, not treated as independent.
    pub dependencies: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecognizerIdentity {
    pub engine: String,
    pub model: Option<String>,
    pub version: Option<String>,
    pub decoder: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecognitionSourceProfile {
    pub id: String,
    pub source: RecognitionSource,
    pub transcription_policy_id: Option<String>,
    pub required_signals: Vec<RecognizerSignal>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateScore {
    pub component: String,
    pub score: RawScore,
    pub dependencies: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateAlignment {
    /// Byte span in the unchanged 1-best, empty for an insertion boundary.
    pub original: ByteSpan,
    /// Byte span in this candidate, empty for a deletion boundary.
    pub candidate: ByteSpan,
    pub edit: CandidateEditKind,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecognitionCandidate {
    pub rank: usize,
    pub text: String,
    pub scores: Vec<CandidateScore>,
    pub alignment: Vec<CandidateAlignment>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateEvidence {
    pub status: RecognitionEvidenceStatus,
    pub hypotheses: Vec<RecognitionCandidate>,
    /// None means the recognizer did not disclose whether its search was truncated.
    pub truncated: Option<bool>,
    pub origin: Option<String>,
    pub reason: Option<String>,
}
impl RecognitionSourceProfile {
    pub fn new(id: impl Into<String>, source: RecognitionSource) -> Self {
        Self {
            id: id.into(),
            source,
            transcription_policy_id: None,
            required_signals: vec![],
        }
    }
}
impl CandidateEvidence {
    /// Preflight all hypothesis sizes before any quadratic alignment allocation.
    pub fn align(&mut self, original: &str) -> Result<(), EvaluationError> {
        if self.status != RecognitionEvidenceStatus::Observed {
            if !self.hypotheses.is_empty() || self.truncated.is_some() || self.origin.is_some() {
                return Err(EvaluationError::InvalidInput(
                    "unavailable candidate payload".into(),
                ));
            }
            return Ok(());
        }
        if self.hypotheses.is_empty()
            || self.hypotheses.len() > MAX_CANDIDATES
            || self.hypotheses[0].text != original
            || self.origin.as_ref().is_none_or(|s| s.trim().is_empty())
            || self
                .hypotheses
                .iter()
                .try_fold(0usize, |n, c| n.checked_add(c.text.len()))
                .is_none_or(|n| n > MAX_CANDIDATE_BYTES)
        {
            return Err(EvaluationError::InvalidInput(
                "invalid or oversized candidate payload".into(),
            ));
        }
        let mut cells = 0usize;
        for (index, candidate) in self.hypotheses.iter().enumerate() {
            cells = cells
                .checked_add(alignment_cells(original, &candidate.text)?)
                .ok_or_else(|| EvaluationError::InvalidInput("alignment budget exceeded".into()))?;
            if cells > MAX_TOTAL_ALIGNMENT_CELLS
                || candidate.rank != index + 1
                || candidate.scores.len() > 16
            {
                return Err(EvaluationError::InvalidInput(
                    "candidate alignment/rank budget exceeded".into(),
                ));
            }
            for score in &candidate.scores {
                score.score.validate()?;
            }
        }
        for candidate in &mut self.hypotheses {
            candidate.alignment = align_recognition_candidate(original, &candidate.text)?;
        }
        Ok(())
    }
    pub fn missing() -> Self {
        Self {
            status: RecognitionEvidenceStatus::Missing,
            hypotheses: vec![],
            truncated: None,
            origin: None,
            reason: Some("candidates_not_provided".into()),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecognitionSourceAnchor {
    pub id: String,
    pub span: ByteSpan,
    /// OCR/ASR geometry/timing is validated by the adapter; core treats it as opaque.
    pub data: serde_json::Value,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecognizerEvidence {
    pub schema_version: String,
    pub source: RecognitionSource,
    pub recognizer: RecognizerIdentity,
    pub profile: RecognitionSourceProfile,
    pub confidences: Vec<ConfidenceObservation>,
    pub candidates: CandidateEvidence,
    pub anchors: Vec<RecognitionSourceAnchor>,
}
fn optional_identity(value: &Option<String>) -> bool {
    value
        .as_ref()
        .is_none_or(|value| !value.trim().is_empty() && value.len() <= 256)
}
fn validate_dependencies(dependencies: &[String]) -> Result<(), EvaluationError> {
    let unique = dependencies
        .iter()
        .collect::<std::collections::HashSet<_>>();
    if dependencies.len() > 32
        || unique.len() != dependencies.len()
        || dependencies
            .iter()
            .any(|d| d.trim().is_empty() || d.len() > 256)
    {
        return Err(EvaluationError::InvalidInput(
            "invalid evidence dependency identities".into(),
        ));
    }
    Ok(())
}
impl RecognizerEvidence {
    /// Called before morphology or model execution and again during report validation.
    pub fn validate(&self, text: &str, source: RecognitionSource) -> Result<(), EvaluationError> {
        if self.schema_version != RECOGNIZER_EVIDENCE_SCHEMA
            || self.source != source
            || self.profile.source != source
            || (self.profile.id.trim().is_empty() || self.profile.id.len() > 256)
            || (self.recognizer.engine.trim().is_empty() || self.recognizer.engine.len() > 256)
            || !optional_identity(&self.profile.transcription_policy_id)
            || self.profile.required_signals.len() > 3
            || self
                .profile
                .required_signals
                .iter()
                .enumerate()
                .any(|(i, signal)| self.profile.required_signals[..i].contains(signal))
            || !optional_identity(&self.recognizer.model)
            || !optional_identity(&self.recognizer.version)
            || !optional_identity(&self.recognizer.decoder)
        {
            return Err(EvaluationError::InvalidInput(
                "recognizer source/profile/identity mismatch".into(),
            ));
        }
        if self.confidences.len() > MAX_SOURCE_OBSERVATIONS
            || self.anchors.len() > MAX_SOURCE_OBSERVATIONS
            || self.candidates.hypotheses.len() > MAX_CANDIDATES
            || self
                .candidates
                .hypotheses
                .iter()
                .try_fold(0usize, |n, c| n.checked_add(c.text.len()))
                .is_none_or(|n| n > MAX_CANDIDATE_BYTES)
        {
            return Err(EvaluationError::InvalidInput(
                "recognizer evidence resource limit exceeded".into(),
            ));
        }
        let mut ids = std::collections::HashSet::new();
        for confidence in &self.confidences {
            confidence.span.validate(text)?;
            if (confidence.id.trim().is_empty() || confidence.id.len() > 256)
                || !ids.insert(&confidence.id)
                || (confidence.status == RecognitionEvidenceStatus::Observed)
                    != confidence.score.is_some()
                || !optional_identity(&confidence.aggregation)
                || !optional_identity(&confidence.reason)
                || confidence.status != RecognitionEvidenceStatus::Observed
                    && confidence.reason.is_none()
                || confidence.granularity == ConfidenceGranularity::Character
                    && text[confidence.span.start()..confidence.span.end()]
                        .chars()
                        .count()
                        != 1
            {
                return Err(EvaluationError::InvalidInput(
                    "invalid confidence observation".into(),
                ));
            }
            if let Some(score) = &confidence.score {
                score.validate()?;
            }
            validate_dependencies(&confidence.dependencies)?;
        }
        ids.clear();
        for anchor in &self.anchors {
            anchor.span.validate(text)?;
            if (anchor.id.trim().is_empty() || anchor.id.len() > 256)
                || !ids.insert(&anchor.id)
                || !anchor.data.is_object()
                || anchor.data.to_string().len() > 4096
            {
                return Err(EvaluationError::InvalidInput(
                    "invalid source anchor".into(),
                ));
            }
        }
        let candidates = &self.candidates;
        if !optional_identity(&candidates.origin)
            || !optional_identity(&candidates.reason)
            || candidates.status != RecognitionEvidenceStatus::Observed
                && candidates.reason.is_none()
        {
            return Err(EvaluationError::InvalidInput(
                "invalid candidate origin".into(),
            ));
        }
        if candidates.status != RecognitionEvidenceStatus::Observed {
            if !candidates.hypotheses.is_empty()
                || candidates.truncated.is_some()
                || candidates.origin.is_some()
            {
                return Err(EvaluationError::InvalidInput(
                    "unavailable candidates cannot contain hypotheses".into(),
                ));
            }
        } else {
            if candidates.hypotheses.is_empty()
                || candidates.hypotheses[0].text != text
                || candidates.origin.is_none()
            {
                return Err(EvaluationError::InvalidInput(
                    "candidate rank 1 must preserve the input and declare its origin".into(),
                ));
            }
            let mut total_cells = 0usize;
            for (index, candidate) in candidates.hypotheses.iter().enumerate() {
                let cells = alignment_cells(text, &candidate.text)?;
                total_cells = total_cells.checked_add(cells).ok_or_else(|| {
                    EvaluationError::InvalidInput("alignment budget exceeded".into())
                })?;
                if total_cells > MAX_TOTAL_ALIGNMENT_CELLS
                    || candidate.rank != index + 1
                    || candidate.scores.len() > 16
                {
                    return Err(EvaluationError::InvalidInput(
                        "invalid candidate ranks/score limits/alignment budget".into(),
                    ));
                }
                let mut components = std::collections::HashSet::new();
                for score in &candidate.scores {
                    if (score.component.trim().is_empty() || score.component.len() > 256)
                        || !components.insert(&score.component)
                    {
                        return Err(EvaluationError::InvalidInput(
                            "invalid candidate score components".into(),
                        ));
                    }
                    score.score.validate()?;
                    validate_dependencies(&score.dependencies)?;
                }
                validate_alignment(text, &candidate.text, &candidate.alignment)?;
            }
        }
        Ok(())
    }
    pub fn missing_required_signals(&self) -> Vec<RecognizerSignal> {
        self.profile
            .required_signals
            .iter()
            .copied()
            .filter(|signal| match signal {
                RecognizerSignal::Confidence => !self
                    .confidences
                    .iter()
                    .any(|c| c.status == RecognitionEvidenceStatus::Observed),
                RecognizerSignal::Candidates => {
                    self.candidates.status != RecognitionEvidenceStatus::Observed
                }
                RecognizerSignal::SourceAnchors => self.anchors.is_empty(),
            })
            .collect()
    }
    pub fn summary(&self) -> serde_json::Value {
        serde_json::json!({
            "confidence_observation_count":self.confidences.len(),
            "observed_confidence_count":self.confidences.iter().filter(|c| c.status == RecognitionEvidenceStatus::Observed).count(),
            "unknown_score_semantics_count":self.confidences.iter().filter(|c| c.score.as_ref().is_some_and(|s| s.meaning == ConfidenceMeaning::Unknown)).count(),
            "candidate_status":self.candidates.status,
            "candidate_count":self.candidates.hypotheses.len(),
            "candidate_disagreement_count":self.candidates.hypotheses.iter().skip(1).filter(|c| c.text != self.candidates.hypotheses[0].text).count(),
            "search_truncated":self.candidates.truncated,
            "anchor_count":self.anchors.len(),
            "missing_required_signals":self.missing_required_signals(),
            "score_normalization":"not_performed",
            "recognition_correctness":"not_estimated"
        })
    }
}
fn alignment_cells(original: &str, candidate: &str) -> Result<usize, EvaluationError> {
    // Identity alignment is linear and does not need a DP matrix.
    if original == candidate {
        return Ok(0);
    }
    let cells = (original.chars().count() + 1).checked_mul(candidate.chars().count() + 1);
    cells.filter(|n| *n <= MAX_ALIGNMENT_CELLS).ok_or_else(|| {
        EvaluationError::InvalidInput("candidate alignment cell limit exceeded".into())
    })
}
fn validate_alignment(
    original: &str,
    candidate: &str,
    alignment: &[CandidateAlignment],
) -> Result<(), EvaluationError> {
    if alignment.len() > original.chars().count() + candidate.chars().count() + 1 {
        return Err(EvaluationError::InvalidInput(
            "candidate alignment output limit exceeded".into(),
        ));
    }
    let (mut a, mut b) = (0, 0);
    for piece in alignment {
        piece.original.validate(original)?;
        piece.candidate.validate(candidate)?;
        let first = &original[piece.original.start()..piece.original.end()];
        let second = &candidate[piece.candidate.start()..piece.candidate.end()];
        let valid_edit = match piece.edit {
            CandidateEditKind::Equal => !first.is_empty() && first == second,
            CandidateEditKind::Substitution => {
                !first.is_empty() && !second.is_empty() && first != second
            }
            CandidateEditKind::Insertion => first.is_empty() && !second.is_empty(),
            CandidateEditKind::Deletion => !first.is_empty() && second.is_empty(),
        };
        if piece.original.start() != a || piece.candidate.start() != b || !valid_edit {
            return Err(EvaluationError::InvalidInput(
                "candidate alignment does not preserve contiguous raw coordinates".into(),
            ));
        }
        a = piece.original.end();
        b = piece.candidate.end();
    }
    if a != original.len() || b != candidate.len() {
        return Err(EvaluationError::InvalidInput(
            "candidate alignment is incomplete".into(),
        ));
    }
    Ok(())
}

/// Deterministic Unicode-scalar Levenshtein alignment. Byte positions refer to raw strings.
/// This is a comparison, not evidence that an alternative candidate is correct.
pub fn align_recognition_candidate(
    original: &str,
    candidate: &str,
) -> Result<Vec<CandidateAlignment>, EvaluationError> {
    alignment_cells(original, candidate)?;
    if original == candidate {
        return Ok(if original.is_empty() {
            vec![]
        } else {
            vec![CandidateAlignment {
                original: ByteSpan::whole(original),
                candidate: ByteSpan::whole(candidate),
                edit: CandidateEditKind::Equal,
            }]
        });
    }
    let a: Vec<_> = original.chars().collect();
    let b: Vec<_> = candidate.chars().collect();
    let width = b.len() + 1;
    let mut matrix = vec![0usize; (a.len() + 1) * width];
    for i in 0..=a.len() {
        matrix[i * width] = i;
    }
    for j in 0..=b.len() {
        matrix[j] = j;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            matrix[i * width + j] = (matrix[(i - 1) * width + j - 1]
                + usize::from(a[i - 1] != b[j - 1]))
            .min(matrix[(i - 1) * width + j] + 1)
            .min(matrix[i * width + j - 1] + 1);
        }
    }
    let offsets = |text: &str| {
        text.char_indices()
            .map(|(offset, _)| offset)
            .chain(std::iter::once(text.len()))
            .collect::<Vec<_>>()
    };
    let ao = offsets(original);
    let bo = offsets(candidate);
    let (mut i, mut j) = (a.len(), b.len());
    let mut result = Vec::new();
    while i > 0 || j > 0 {
        let (ni, nj, edit) = if i > 0
            && j > 0
            && matrix[i * width + j]
                == matrix[(i - 1) * width + j - 1] + usize::from(a[i - 1] != b[j - 1])
        {
            (
                i - 1,
                j - 1,
                if a[i - 1] == b[j - 1] {
                    CandidateEditKind::Equal
                } else {
                    CandidateEditKind::Substitution
                },
            )
        } else if i > 0 && matrix[i * width + j] == matrix[(i - 1) * width + j] + 1 {
            (i - 1, j, CandidateEditKind::Deletion)
        } else {
            (i, j - 1, CandidateEditKind::Insertion)
        };
        result.push(CandidateAlignment {
            original: ByteSpan::new(original, ao[ni], ao[i])?,
            candidate: ByteSpan::new(candidate, bo[nj], bo[j])?,
            edit,
        });
        i = ni;
        j = nj;
    }
    result.reverse();
    validate_alignment(original, candidate, &result)?;
    Ok(result)
}
