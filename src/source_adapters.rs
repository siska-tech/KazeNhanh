//! Source-specific geometry, timing and granularity checks outside the core.
use kaze_nhanh_core::*;

#[derive(Clone, Debug)]
pub struct OcrRegion {
    pub id: String,
    pub span: ByteSpan,
    /// Zero-based page index; [left, top, right, bottom] in caller-declared pixel space.
    pub page: usize,
    pub bbox: [f64; 4],
}
#[derive(Clone, Debug)]
pub struct AsrRegion {
    pub id: String,
    pub span: ByteSpan,
    /// Seconds relative to the caller's recording/session origin.
    pub start_seconds: f64,
    pub end_seconds: f64,
}
#[derive(Clone, Debug)]
pub struct OcrEvidencePayload {
    pub recognizer: RecognizerIdentity,
    pub profile: RecognitionSourceProfile,
    pub confidences: Vec<ConfidenceObservation>,
    pub candidates: CandidateEvidence,
    pub regions: Vec<OcrRegion>,
}
#[derive(Clone, Debug)]
pub struct AsrEvidencePayload {
    pub recognizer: RecognizerIdentity,
    pub profile: RecognitionSourceProfile,
    pub confidences: Vec<ConfidenceObservation>,
    pub candidates: CandidateEvidence,
    pub regions: Vec<AsrRegion>,
}

pub fn adapt_ocr_evidence(
    text: &str,
    mut payload: OcrEvidencePayload,
) -> Result<RecognizerEvidence, EvaluationError> {
    if payload.regions.len() > MAX_SOURCE_OBSERVATIONS
        || payload.confidences.len() > MAX_SOURCE_OBSERVATIONS
        || payload
            .confidences
            .iter()
            .any(|c| c.granularity == ConfidenceGranularity::Utterance)
    {
        return Err(EvaluationError::InvalidInput(
            "invalid OCR granularity/resource limits".into(),
        ));
    }
    let mut anchors = Vec::new();
    for region in payload.regions {
        let [left, top, right, bottom] = region.bbox;
        if region.bbox.iter().any(|v| !v.is_finite() || *v < 0.0) || left >= right || top >= bottom
        {
            return Err(EvaluationError::InvalidInput(
                "invalid OCR pixel bbox".into(),
            ));
        }
        region.span.validate(text)?;
        anchors.push(RecognitionSourceAnchor {
            id: region.id, span: region.span,
            data: serde_json::json!({"kind":"ocr_bbox","page":region.page,"bbox":region.bbox,"coordinate_unit":"pixel"}),
        });
    }
    payload.candidates.align(text)?;
    let result = RecognizerEvidence {
        schema_version: RECOGNIZER_EVIDENCE_SCHEMA.into(),
        source: RecognitionSource::Ocr,
        recognizer: payload.recognizer,
        profile: payload.profile,
        confidences: payload.confidences,
        candidates: payload.candidates,
        anchors,
    };
    result.validate(text, RecognitionSource::Ocr)?;
    Ok(result)
}
pub fn adapt_asr_evidence(
    text: &str,
    mut payload: AsrEvidencePayload,
) -> Result<RecognizerEvidence, EvaluationError> {
    if payload.regions.len() > MAX_SOURCE_OBSERVATIONS
        || payload.confidences.len() > MAX_SOURCE_OBSERVATIONS
        || payload
            .confidences
            .iter()
            .any(|c| c.granularity == ConfidenceGranularity::Line)
    {
        return Err(EvaluationError::InvalidInput(
            "invalid ASR granularity/resource limits".into(),
        ));
    }
    let mut anchors = Vec::new();
    for region in payload.regions {
        if !region.start_seconds.is_finite()
            || !region.end_seconds.is_finite()
            || region.start_seconds < 0.0
            || region.end_seconds < region.start_seconds
            || region.span.start() != region.span.end()
                && region.start_seconds == region.end_seconds
        {
            return Err(EvaluationError::InvalidInput(
                "invalid ASR time interval".into(),
            ));
        }
        region.span.validate(text)?;
        anchors.push(RecognitionSourceAnchor {
            id: region.id, span: region.span,
            data: serde_json::json!({"kind":"asr_time","start_seconds":region.start_seconds,"end_seconds":region.end_seconds}),
        });
    }
    payload.candidates.align(text)?;
    let result = RecognizerEvidence {
        schema_version: RECOGNIZER_EVIDENCE_SCHEMA.into(),
        source: RecognitionSource::Asr,
        recognizer: payload.recognizer,
        profile: payload.profile,
        confidences: payload.confidences,
        candidates: payload.candidates,
        anchors,
    };
    result.validate(text, RecognitionSource::Asr)?;
    Ok(result)
}

mod confidence_review;
pub use confidence_review::*;
