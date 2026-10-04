//! Contract fixtures are synthetic; no recognition-quality claims are made.
use kaze_nhanh::source_adapters::*;
use kaze_nhanh::*;
use std::sync::Arc;

fn identity() -> RecognizerIdentity {
    RecognizerIdentity {
        engine: "fixture-recognizer".into(),
        model: None,
        version: None,
        decoder: None,
    }
}
fn score(value: f64, meaning: ConfidenceMeaning) -> RawScore {
    RawScore {
        value,
        meaning,
        direction: ConfidenceDirection::HigherIsBetter,
        range: None,
        calibration_id: None,
        target: None,
    }
}
fn confidence(text: &str, granularity: ConfidenceGranularity) -> ConfidenceObservation {
    ConfidenceObservation {
        id: "confidence-1".into(),
        span: ByteSpan::whole(text),
        granularity,
        status: RecognitionEvidenceStatus::Observed,
        score: Some(score(0.975, ConfidenceMeaning::EngineScore)),
        aggregation: None,
        reason: None,
        dependencies: vec!["shared-decoder.v1".into()],
    }
}
fn candidates(original: &str, alternative: &str) -> CandidateEvidence {
    CandidateEvidence {
        status: RecognitionEvidenceStatus::Observed,
        hypotheses: [original, alternative]
            .into_iter()
            .enumerate()
            .map(|(i, text)| RecognitionCandidate {
                rank: i + 1,
                text: text.into(),
                scores: vec![CandidateScore {
                    component: "decoder".into(),
                    score: score(-(i as f64) - 1.0, ConfidenceMeaning::LogProbability),
                    dependencies: vec!["shared-decoder.v1".into()],
                }],
                alignment: vec![],
            })
            .collect(),
        truncated: Some(true),
        origin: Some("synthetic-nbest-contract-fixture".into()),
        reason: None,
    }
}
fn ocr(text: &str) -> OcrEvidencePayload {
    OcrEvidencePayload {
        recognizer: identity(),
        profile: RecognitionSourceProfile::new("fixture.ocr.v1", RecognitionSource::Ocr),
        confidences: vec![confidence(text, ConfidenceGranularity::Line)],
        candidates: CandidateEvidence::missing(),
        regions: vec![OcrRegion {
            id: "region-1".into(),
            span: ByteSpan::whole(text),
            page: 0,
            bbox: [1.0, 2.0, 10.0, 20.0],
        }],
    }
}
fn asr(text: &str) -> AsrEvidencePayload {
    AsrEvidencePayload {
        recognizer: identity(),
        profile: RecognitionSourceProfile::new("fixture.asr.v1", RecognitionSource::Asr),
        confidences: vec![confidence(text, ConfidenceGranularity::Utterance)],
        candidates: CandidateEvidence::missing(),
        regions: vec![AsrRegion {
            id: "utterance-1".into(),
            span: ByteSpan::whole(text),
            start_seconds: 0.5,
            end_seconds: 2.0,
        }],
    }
}
struct Analyzer;
impl MorphAnalyzer for Analyzer {
    fn analyze(&self, _: &str) -> Result<MorphAnalysis, EvaluationError> {
        Ok(MorphAnalysis {
            morphemes: vec![],
            provenance: vec![],
        })
    }
}
fn engine() -> RecognitionEngine {
    RecognitionEngine::new(Arc::new(Analyzer), RecognitionConfig::default()).unwrap()
}

#[test]
fn ocr_and_asr_preserve_scores_source_positions_and_profiles_without_low_risk() {
    for source in [RecognitionSource::Ocr, RecognitionSource::Asr] {
        let text = "今日の天気";
        let evidence = match source {
            RecognitionSource::Ocr => adapt_ocr_evidence(text, ocr(text)).unwrap(),
            RecognitionSource::Asr => adapt_asr_evidence(text, asr(text)).unwrap(),
        };
        let mut input = RecognitionInput::new(text, source, "document", "segment");
        input.recognizer_evidence = Some(&evidence);
        let report = engine().evaluate_recognition(input).unwrap();
        assert_eq!(report.recognizer_evidence, Some(evidence.clone()));
        assert_eq!(report.profile_id, evidence.profile.id);
        assert_eq!(report.original_text, text);
        assert_eq!(report.recognition_risk.value, None);
        assert_eq!(report.decision, RecognitionDecision::Undetermined);
        assert_eq!(report.metrics.slm_calls, 0);
        let row = report
            .evidence
            .iter()
            .find(|e| e.kind == RecognitionEvidenceKind::Recognizer)
            .unwrap();
        assert_eq!(row.status, RecognitionEvidenceStatus::Observed);
        assert_eq!(row.value, Some(evidence.summary()));
        let decoded: RecognitionReport =
            serde_json::from_str(&serde_json::to_string(&report).unwrap()).unwrap();
        decoded.validate().unwrap();
        let mut forged = decoded;
        forged.recognizer_evidence.as_mut().unwrap().confidences[0]
            .score
            .as_mut()
            .unwrap()
            .value = 2.0;
        // Valid engine score changed without corresponding count changes is still raw evidence;
        // replace a posterior to require a range failure instead of claiming calibration.
        forged.recognizer_evidence.as_mut().unwrap().confidences[0]
            .score
            .as_mut()
            .unwrap()
            .meaning = ConfidenceMeaning::Posterior;
        assert!(forged.validate().is_err());
    }
}
#[test]
fn missing_zero_and_unknown_confidence_are_distinct_and_required_signals_are_reported() {
    let text = "猫";
    let mut payload = ocr(text);
    payload.profile.required_signals =
        vec![RecognizerSignal::Confidence, RecognizerSignal::Candidates];
    payload.confidences[0].score = None;
    payload.confidences[0].status = RecognitionEvidenceStatus::Missing;
    payload.confidences[0].reason = Some("engine_did_not_provide_confidence".into());
    let missing = adapt_ocr_evidence(text, payload).unwrap();
    assert_eq!(
        missing.missing_required_signals(),
        vec![RecognizerSignal::Confidence, RecognizerSignal::Candidates]
    );
    let mut input = RecognitionInput::new(text, RecognitionSource::Ocr, "d", "s");
    input.recognizer_evidence = Some(&missing);
    let report = engine().evaluate_recognition(input).unwrap();
    assert!(report
        .reasons
        .contains(&"required_source_signals_missing".into()));
    let mut payload = ocr(text);
    payload.confidences[0].score = Some(score(0.0, ConfidenceMeaning::Unknown));
    let zero = adapt_ocr_evidence(text, payload).unwrap();
    assert_eq!(zero.confidences[0].score.as_ref().unwrap().value, 0.0);
    assert_eq!(zero.summary()["unknown_score_semantics_count"], 1);
    assert_eq!(zero.summary()["observed_confidence_count"], 1);
}
#[test]
fn nbest_uses_raw_unicode_coordinates_and_retains_truncation_and_dependencies() {
    let text = "猫🙂は";
    let mut payload = asr(text);
    payload.candidates = candidates(text, "猫は!");
    let evidence = adapt_asr_evidence(text, payload).unwrap();
    assert_eq!(evidence.candidates.truncated, Some(true));
    assert_eq!(evidence.summary()["candidate_disagreement_count"], 1);
    let alternative = &evidence.candidates.hypotheses[1];
    assert_eq!(
        alternative.scores[0].dependencies,
        vec!["shared-decoder.v1"]
    );
    for piece in &alternative.alignment {
        piece.original.validate(text).unwrap();
        piece.candidate.validate(&alternative.text).unwrap();
    }
    assert_eq!(
        alternative.alignment.last().unwrap().original.end(),
        text.len()
    );
    assert_eq!(
        alternative.alignment.last().unwrap().candidate.end(),
        alternative.text.len()
    );
    assert!(alternative
        .alignment
        .iter()
        .any(|p| p.edit != CandidateEditKind::Equal));
    assert_eq!(evidence.summary()["score_normalization"], "not_performed");
    let mut input = RecognitionInput::new(text, RecognitionSource::Asr, "d", "s");
    input.recognizer_evidence = Some(&evidence);
    let report = engine().evaluate_recognition(input).unwrap();
    assert_eq!(report.decision, RecognitionDecision::Undetermined);
    let mut forged = report;
    forged
        .recognizer_evidence
        .as_mut()
        .unwrap()
        .candidates
        .hypotheses[1]
        .alignment[0]
        .original = ByteSpan::whole(text);
    assert!(forged.validate().is_err());
}
#[test]
fn insertion_and_deletion_use_empty_boundary_spans() {
    let inserted = align_recognition_candidate("猫", "猫🙂").unwrap();
    let insertion = inserted
        .iter()
        .find(|p| p.edit == CandidateEditKind::Insertion)
        .unwrap();
    assert_eq!(insertion.original.start(), "猫".len());
    assert_eq!(insertion.original.end(), "猫".len());
    let deleted = align_recognition_candidate("猫🙂", "猫").unwrap();
    let deletion = deleted
        .iter()
        .find(|p| p.edit == CandidateEditKind::Deletion)
        .unwrap();
    assert_eq!(deletion.candidate.start(), "猫".len());
    assert_eq!(deletion.candidate.end(), "猫".len());
    assert!(align_recognition_candidate("", "").unwrap().is_empty());
    assert_eq!(
        align_recognition_candidate("猫", "猫").unwrap()[0].edit,
        CandidateEditKind::Equal
    );
}
#[test]
fn source_specific_geometry_timing_and_granularity_are_checked() {
    let text = "猫";
    for bbox in [
        [2.0, 1.0, 1.0, 2.0],
        [0.0, 0.0, f64::NAN, 2.0],
        [-1.0, 0.0, 1.0, 2.0],
    ] {
        let mut payload = ocr(text);
        payload.regions[0].bbox = bbox;
        assert!(adapt_ocr_evidence(text, payload).is_err());
    }
    let mut payload = asr(text);
    payload.regions[0].end_seconds = 0.1;
    assert!(adapt_asr_evidence(text, payload).is_err());
    let mut payload = asr(text);
    payload.confidences[0].granularity = ConfidenceGranularity::Line;
    assert!(adapt_asr_evidence(text, payload).is_err());
    let mut payload = ocr(text);
    payload.profile.source = RecognitionSource::Asr;
    assert!(adapt_ocr_evidence(text, payload).is_err());
}
#[test]
fn rejects_nonfinite_mislabelled_scores_bad_rank_schema_and_alignment_resource_limits() {
    let text = "猫";
    for bad in [f64::NAN, f64::INFINITY, -0.1, 1.1] {
        let mut payload = ocr(text);
        payload.confidences[0].score = Some(score(bad, ConfidenceMeaning::Posterior));
        assert!(adapt_ocr_evidence(text, payload).is_err());
    }
    let mut payload = ocr(text);
    payload.confidences[0].score = Some(score(1.0, ConfidenceMeaning::LogProbability));
    assert!(adapt_ocr_evidence(text, payload).is_err());
    let mut payload = ocr(text);
    payload.candidates = candidates(text, "犬");
    payload.candidates.hypotheses[1].rank = 1;
    assert!(adapt_ocr_evidence(text, payload).is_err());
    let mut payload = ocr(text);
    payload.candidates = candidates("別の原文", "犬");
    assert!(adapt_ocr_evidence(text, payload).is_err());
    let mut evidence = adapt_ocr_evidence(text, ocr(text)).unwrap();
    evidence.schema_version = "unknown".into();
    assert!(evidence.validate(text, RecognitionSource::Ocr).is_err());
    assert!(align_recognition_candidate(&"猫".repeat(600), &"犬".repeat(600)).is_err());
    let mut payload = ocr(text);
    payload.candidates = candidates(text, "犬");
    payload.candidates.hypotheses = (1..=MAX_CANDIDATES + 1)
        .map(|rank| RecognitionCandidate {
            rank,
            text: text.into(),
            scores: vec![],
            alignment: vec![],
        })
        .collect();
    assert!(adapt_ocr_evidence(text, payload).is_err());
}
#[test]
fn mismatched_source_is_rejected_before_morphology() {
    struct PanicAnalyzer;
    impl MorphAnalyzer for PanicAnalyzer {
        fn analyze(&self, _: &str) -> Result<MorphAnalysis, EvaluationError> {
            panic!("must not execute")
        }
    }
    let evidence = adapt_ocr_evidence("猫", ocr("猫")).unwrap();
    let engine =
        RecognitionEngine::new(Arc::new(PanicAnalyzer), RecognitionConfig::default()).unwrap();
    let mut input = RecognitionInput::new("猫", RecognitionSource::Asr, "d", "s");
    input.recognizer_evidence = Some(&evidence);
    assert!(engine.evaluate_recognition(input).is_err());
}

#[test]
fn rejects_unavailable_values_character_span_mismatch_summary_tampering_and_old_schema() {
    let text = "猫犬";
    let mut payload = ocr(text);
    payload.confidences[0].status = RecognitionEvidenceStatus::Missing;
    assert!(adapt_ocr_evidence(text, payload).is_err());
    let mut payload = ocr(text);
    payload.confidences[0].granularity = ConfidenceGranularity::Character;
    assert!(adapt_ocr_evidence(text, payload).is_err());
    let mut payload = ocr(text);
    payload.confidences[0].score.as_mut().unwrap().range = Some([0.0, 0.5]);
    assert!(adapt_ocr_evidence(text, payload).is_err());
    let evidence = adapt_ocr_evidence(text, ocr(text)).unwrap();
    let mut input = RecognitionInput::new(text, RecognitionSource::Ocr, "d", "s");
    input.recognizer_evidence = Some(&evidence);
    let report = engine().evaluate_recognition(input).unwrap();
    let mut forged = report.clone();
    forged
        .evidence
        .iter_mut()
        .find(|e| e.kind == RecognitionEvidenceKind::Recognizer)
        .unwrap()
        .value = Some(serde_json::json!({"confidence_observation_count":999}));
    assert!(forged.validate().is_err());
    let mut forged = report.clone();
    forged.coverage.source_completeness_assessed = true;
    assert!(forged.validate().is_err());
    let mut forged = report;
    forged.schema_version = "kzn.recognition.v1".into();
    assert!(forged.validate().is_err());
}
#[test]
fn total_alignment_budget_is_checked_before_aligning_the_set() {
    let text = "猫".repeat(450);
    let alternative = "犬".repeat(450);
    let mut payload = ocr(&text);
    payload.candidates = candidates(&text, &alternative);
    payload.candidates.hypotheses = (1..=7)
        .map(|rank| RecognitionCandidate {
            rank,
            text: if rank == 1 {
                text.clone()
            } else {
                alternative.clone()
            },
            scores: vec![],
            alignment: vec![],
        })
        .collect();
    assert!(adapt_ocr_evidence(&text, payload).is_err());
}

#[test]
fn opt_in_candidate_disagreement_reviews_fluent_ocr_asr_without_probability() {
    for source in [RecognitionSource::Ocr, RecognitionSource::Asr] {
        let text = "料金は100円です";
        let mut evidence = match source {
            RecognitionSource::Ocr => {
                let mut p = ocr(text);
                p.candidates = candidates(text, "料金は700円です");
                adapt_ocr_evidence(text, p).unwrap()
            }
            RecognitionSource::Asr => {
                let mut p = asr(text);
                p.candidates = candidates(text, "料金は700円です");
                adapt_asr_evidence(text, p).unwrap()
            }
        };
        // Even high confidence must not erase supplied alternative uncertainty.
        evidence.confidences[0].score = Some(score(0.999, ConfidenceMeaning::Posterior));
        let mut input = RecognitionInput::new(text, source, "d", "s");
        input.recognizer_evidence = Some(&evidence);
        assert_eq!(
            engine()
                .evaluate_recognition(input.clone())
                .unwrap()
                .decision,
            RecognitionDecision::Undetermined
        );
        let report = engine()
            .with_candidate_disagreement_review()
            .evaluate_recognition(input)
            .unwrap();
        assert_eq!(report.decision, RecognitionDecision::Review);
        assert_eq!(report.decision_policy.id, CANDIDATE_REVIEW_POLICY_ID);
        assert_eq!(report.recognition_risk.value, None);
        assert_eq!(
            report.recognition_risk.status,
            RecognitionAssessmentStatus::InsufficientEvidence
        );
        assert_eq!(report.evidence_adequacy, EvidenceAdequacy::Limited);
        assert_eq!(report.metrics.slm_calls, 0);
        let finding = report
            .findings
            .iter()
            .find(|f| f.issue.code == "candidate_disagreement")
            .unwrap();
        assert_eq!(finding.issue.span, ByteSpan::new(text, 9, 10).unwrap());
        assert_eq!(finding.issue.evidence["candidate_rank"], 2);
        assert_eq!(finding.issue.evidence["scores_used"], false);
        let decoded: RecognitionReport =
            serde_json::from_str(&serde_json::to_string(&report).unwrap()).unwrap();
        decoded.validate().unwrap();
        let mut forged = report.clone();
        forged.findings.clear();
        assert!(forged.validate().is_err());
        let mut forged = report.clone();
        forged.findings[0].issue.evidence["candidate_rank"] = serde_json::json!(1);
        assert!(forged.validate().is_err());
        let mut forged = report.clone();
        forged.decision_policy.id = "kzn.recognition.abstain.v1".into();
        assert!(forged.validate().is_err());
        let mut forged = report;
        forged.decision = RecognitionDecision::Undetermined;
        assert!(forged.validate().is_err());
    }
}

#[test]
fn absent_identical_and_truncated_candidates_do_not_establish_low_risk() {
    for text in ["体系キープ", "", "今日はいい天気です"] {
        for available in [false, true] {
            let mut p = ocr(text);
            p.regions.clear();
            p.confidences.clear();
            if available {
                p.candidates = candidates(text, text);
            }
            let evidence = adapt_ocr_evidence(text, p).unwrap();
            let mut input = RecognitionInput::new(text, RecognitionSource::Ocr, "d", "s");
            input.recognizer_evidence = Some(&evidence);
            let report = engine()
                .with_candidate_disagreement_review()
                .evaluate_recognition(input)
                .unwrap();
            assert_eq!(report.decision, RecognitionDecision::Undetermined);
            assert_eq!(report.recognition_risk.value, None);
            assert!(report.findings.is_empty());
        }
    }
}

#[test]
fn candidate_review_preserves_insertion_boundaries_deletions_and_raw_variants() {
    for (text, alt, start, end) in [
        ("猫", "猫🙂", 3, 3),
        ("猫🙂", "猫", 3, 7),
        ("", "🙂", 0, 0),
        ("過す", "過ごす", 3, 3),
        ("猫", "猫。", 3, 3),
    ] {
        let mut p = asr(text);
        p.regions.clear();
        p.confidences.clear();
        p.candidates = candidates(text, alt);
        let evidence = adapt_asr_evidence(text, p).unwrap();
        let mut input = RecognitionInput::new(text, RecognitionSource::Asr, "d", "s");
        input.recognizer_evidence = Some(&evidence);
        let report = engine()
            .with_candidate_disagreement_review()
            .evaluate_recognition(input)
            .unwrap();
        assert_eq!(report.decision, RecognitionDecision::Review);
        assert_eq!(
            report.findings[0].issue.span,
            ByteSpan::new(text, start, end).unwrap()
        );
        assert_eq!(report.original_text, text);
        assert_eq!(
            report.recognizer_evidence.unwrap().candidates.hypotheses[1].text,
            alt
        );
    }
}

#[test]
fn candidate_review_is_bounded_and_keeps_independent_primary_findings() {
    let text = "猫猫猫猫猫猫";
    let mut p = ocr(text);
    p.candidates = candidates(text, "犬");
    p.candidates.hypotheses = (0..MAX_CANDIDATES)
        .map(|i| RecognitionCandidate {
            rank: i + 1,
            text: if i == 0 {
                text.into()
            } else {
                format!("犬{i}")
            },
            scores: vec![],
            alignment: vec![],
        })
        .collect();
    let evidence = adapt_ocr_evidence(text, p).unwrap();
    let mut input = RecognitionInput::new(text, RecognitionSource::Ocr, "d", "s");
    input.recognizer_evidence = Some(&evidence);
    let report = engine()
        .with_candidate_disagreement_review()
        .evaluate_recognition(input)
        .unwrap();
    assert_eq!(
        report
            .findings
            .iter()
            .filter(|f| f.issue.code == "candidate_disagreement")
            .count(),
        MAX_CANDIDATES - 1
    );
    assert!(report
        .findings
        .iter()
        .any(|f| f.issue.code != "candidate_disagreement"));
    assert!(report
        .reasons
        .contains(&"primary_findings_require_review".into()));
    assert_eq!(report.metrics.slm_calls, 0);
}
