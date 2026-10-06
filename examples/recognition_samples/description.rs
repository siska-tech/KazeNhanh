//! Observation-only CTC confidence description; contains no decision threshold.
use super::*;
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Description {
    pub schema_version: String,
    pub expected_engine: String,
    pub expected_scale: String,
    pub recognizer: RecognizerIdentity,
    pub profile: RecognitionSourceProfile,
    pub domain: String,
}
impl Description {
    pub fn validate(&self) -> Result<(), Box<dyn std::error::Error>> {
        if self.schema_version != "kzn.ocr.ctc_description.v1"
            || self.expected_engine.trim().is_empty()
            || self.expected_scale
                != "mean CTC emitted-token probability, char-weighted over boxes; null when no text"
            || self.domain.trim().is_empty()
            || self.domain.len() > 256
            || self.profile.source != RecognitionSource::Ocr
        {
            return Err("invalid OCR CTC description".into());
        }
        self.apply(&serde_json::json!({"source":"ocr","engine":self.expected_engine,"confidence_scale":self.expected_scale}),"",None)?;
        Ok(())
    }
    pub fn apply(
        &self,
        sample: &serde_json::Value,
        text: &str,
        raw: Option<f64>,
    ) -> Result<RecognizerEvidence, Box<dyn std::error::Error>> {
        if sample["source"] != "ocr"
            || sample["engine"].as_str() != Some(self.expected_engine.as_str())
            || sample["confidence_scale"].as_str() != Some(self.expected_scale.as_str())
        {
            return Err("OCR source description binding mismatch".into());
        }
        let evidence = adapt_ocr_evidence(
            text,
            OcrEvidencePayload {
                recognizer: self.recognizer.clone(),
                profile: self.profile.clone(),
                confidences: vec![ConfidenceObservation {
                    id: "recognizer-segment-confidence".into(),
                    span: ByteSpan::whole(text),
                    granularity: ConfidenceGranularity::Segment,
                    status: if raw.is_some() {
                        RecognitionEvidenceStatus::Observed
                    } else {
                        RecognitionEvidenceStatus::Missing
                    },
                    score: raw.map(|value| RawScore {
                        value,
                        meaning: ConfidenceMeaning::EngineScore,
                        direction: ConfidenceDirection::HigherIsBetter,
                        range: Some([0.0, 1.0]),
                        calibration_id: None,
                        target: Some("emitted-token-confidence.box-char-weighted".into()),
                    }),
                    aggregation: Some("ctc-emitted-mean.box-char-weighted.v1".into()),
                    reason: raw
                        .is_none()
                        .then(|| "recognizer_confidence_not_provided".into()),
                    dependencies: vec!["recognizer-decoder".into()],
                }],
                candidates: CandidateEvidence::missing(),
                regions: vec![],
            },
        )?;
        evidence.validate(text, RecognitionSource::Ocr)?;
        Ok(evidence)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn description_preserves_missing_and_zero_and_rejects_wrong_source_or_scale() {
        let value = serde_json::json!({"schema_version":"kzn.ocr.ctc_description.v1","expected_engine":"fixture","expected_scale":"mean CTC emitted-token probability, char-weighted over boxes; null when no text","recognizer":{"engine":"fixture","model":null,"version":null,"decoder":null},"profile":{"id":"fixture.ocr","source":"ocr","transcription_policy_id":"raw.v1","required_signals":["confidence"]},"domain":"fixture"});
        let d: Description = serde_json::from_value(value.clone()).unwrap();
        d.validate().unwrap();
        let sample = serde_json::json!({"source":"ocr","engine":d.expected_engine,"confidence_scale":d.expected_scale});
        assert!(d.apply(&sample, "text", None).unwrap().confidences[0]
            .score
            .is_none());
        assert_eq!(
            d.apply(&sample, "text", Some(0.0)).unwrap().confidences[0]
                .score
                .as_ref()
                .unwrap()
                .value,
            0.0
        );
        assert!(d.apply(&sample, "text", Some(1.01)).is_err());
        for key in ["source", "engine", "confidence_scale"] {
            let mut bad = sample.clone();
            bad[key] = serde_json::json!("wrong");
            assert!(d.apply(&bad, "text", Some(0.5)).is_err());
        }
        let mut bad = value;
        bad["threshold"] = serde_json::json!(0.5);
        assert!(serde_json::from_value::<Description>(bad).is_err());
    }
}
