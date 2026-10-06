//! Offline feature probes, never runtime policies or calibrated probabilities.
use super::*;
use kaze_nhanh::{RecognitionEvidenceKind, RecognitionEvidenceStatus, StatisticsObservation};

#[derive(Default)]
struct ProbeCounts {
    flagged: [usize; 3],
    not_flagged: [usize; 3],
    unavailable: [usize; 3],
}
impl ProbeCounts {
    fn add(&mut self, flag: Option<bool>, mismatch: Option<bool>) {
        let column = match mismatch {
            Some(false) => 0,
            Some(true) => 1,
            None => 2,
        };
        let row = match flag {
            Some(true) => &mut self.flagged,
            Some(false) => &mut self.not_flagged,
            None => &mut self.unavailable,
        };
        row[column] += 1;
    }
    fn summary(&self) -> Value {
        let matches = self.flagged[0] + self.not_flagged[0] + self.unavailable[0];
        let errors = self.flagged[1] + self.not_flagged[1] + self.unavailable[1];
        json!({"flagged":self.flagged,"not_flagged":self.not_flagged,"unavailable":self.unavailable,
        "confirmed_metrics":{
            "flag_precision":ratio(self.flagged[1],self.flagged[0]+self.flagged[1]),
            "flag_recall_all_confirmed_errors":ratio(self.flagged[1],errors),
            "false_flag_rate_all_confirmed_matches":ratio(self.flagged[0],matches),
            "evaluable_coverage":ratio(self.flagged[0]+self.flagged[1]+self.not_flagged[0]+self.not_flagged[1],matches+errors)
        }})
    }
}
fn conjunction(left: Option<bool>, right: Option<bool>) -> Option<bool> {
    // Strict applicability: even false AND missing stays unavailable.
    left.zip(right).map(|(a, b)| a && b)
}
fn probes(report: &RecognitionReport) -> Result<[Option<bool>; 5]> {
    let stats: Option<StatisticsObservation> = report
        .evidence
        .iter()
        .find(|e| {
            e.kind == RecognitionEvidenceKind::LexicalStatistics
                && e.status == RecognitionEvidenceStatus::Observed
        })
        .map(|e| serde_json::from_value(e.value.clone().expect("validated observed evidence")))
        .transpose()?;
    let sparse = stats.as_ref().map(|s| {
        s.oov_count > 0 && s.character_pairs.unseen_count > 0 && s.word_pairs.unseen_count > 0
    });
    let pos = stats
        .as_ref()
        .and_then(|s| s.pos.as_ref())
        .and_then(|p|
        // Partial POS coverage is not silently treated as a fully negative observation.
        (p.missing_pair_count == 0).then_some(p).and_then(|p| p.frequencies.as_ref()))
        .and_then(|f| (f.unit_count > 0).then_some(f.unseen_count > 0));
    let change = report
        .string_features
        .as_ref()
        .and_then(|s| (s.scalar_count >= 2).then_some(s.transition_count > 0));
    Ok([
        sparse,
        pos,
        change,
        conjunction(sparse, pos),
        conjunction(pos, change),
    ])
}
const NAMES: [&str; 5] = [
    "sparse_conjunction",
    "unseen_pos",
    "class_change",
    "sparse_and_pos",
    "pos_and_class_change",
];
#[derive(Default)]
pub(super) struct FeatureComparison {
    counts: [ProbeCounts; 5],
}
impl FeatureComparison {
    pub(super) fn add(&mut self, report: &RecognitionReport, mismatch: Option<bool>) -> Result<()> {
        for (counts, flag) in self.counts.iter_mut().zip(probes(report)?) {
            counts.add(flag, mismatch);
        }
        Ok(())
    }
    pub(super) fn summary(&self) -> Value {
        let rows: BTreeMap<_, _> = NAMES
            .into_iter()
            .zip(self.counts.iter().map(ProbeCounts::summary))
            .collect();
        json!({"method":"offline_feature_probes.v1","runtime_policy_changed":false,"quality_accepted":false,
            "columns":["confirmed_match","confirmed_mismatch","unconfirmed"],"probes":rows,
            "definitions":{
                "sparse_conjunction":"oov_count>0 AND unseen_character_pairs>0 AND unseen_word_pairs>0",
                "unseen_pos":"complete input POS coverage AND usable corpus POS pairs AND input POS pairs>0; flag unseen_pos>0",
                "class_change":"at least two raw scalars; flag transition_count>0",
                "sparse_and_pos":"strict-applicability conjunction of sparse_conjunction and unseen_pos",
                "pos_and_class_change":"strict-applicability conjunction of unseen_pos and class_change"
            },
            "limitations":["flags are hypothetical feature conditions, not RecognitionDecision or confirmed errors",
                "missing/partial evidence stays unavailable, including conjunctions with a false operand",
                "recall denominator includes unavailable confirmed errors; unconfirmed labels excluded",
                "correlated evidence; no independence or held-out quality claim",
                "class changes occur in ordinary Japanese and product codes; not an anomaly rule"]})
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strict_conjunction_keeps_missingness() {
        for left in [None, Some(false), Some(true)] {
            for right in [None, Some(false), Some(true)] {
                let expected = match (left, right) {
                    (Some(a), Some(b)) => Some(a && b),
                    _ => None,
                };
                assert_eq!(conjunction(left, right), expected);
            }
        }
    }
    #[test]
    fn unavailable_and_unconfirmed_do_not_inflate_quality() {
        let mut c = ProbeCounts::default();
        c.add(Some(true), Some(true));
        c.add(None, Some(true));
        c.add(Some(true), Some(false));
        c.add(Some(false), Some(false));
        c.add(Some(true), None);
        let s = c.summary();
        assert_eq!(s["flagged"], json!([1, 1, 1]));
        assert_eq!(s["unavailable"], json!([0, 1, 0]));
        assert_eq!(
            s["confirmed_metrics"]["flag_recall_all_confirmed_errors"],
            0.5
        );
        assert_eq!(s["confirmed_metrics"]["flag_precision"], 0.5);
        assert_eq!(s["confirmed_metrics"]["evaluable_coverage"], 0.75);
        assert!(ProbeCounts::default().summary()["confirmed_metrics"]["flag_precision"].is_null());
    }
}

#[cfg(test)]
mod report_tests {
    use super::*;
    use kaze_nhanh::*;
    struct Scalars;
    impl MorphAnalyzer for Scalars {
        fn analyze(&self, text: &str) -> std::result::Result<MorphAnalysis, EvaluationError> {
            Ok(MorphAnalysis {
                provenance: vec![ArtifactIdentity {
                    component: "analyzer".into(),
                    id: "scalar.fixture".into(),
                    sha256: None,
                }],
                morphemes: text
                    .char_indices()
                    .map(|(i, c)| Morpheme {
                        surface: c.to_string(),
                        span: ByteSpan::new(text, i, i + c.len_utf8()).unwrap(),
                        dictionary_form: c.to_string(),
                        normalized_form: c.to_string(),
                        reading: String::new(),
                        part_of_speech: if c == '?' {
                            vec![]
                        } else {
                            vec![if c.is_ascii_digit() {
                                "digit"
                            } else {
                                "letter"
                            }
                            .into()]
                        },
                        is_oov: c != 'a',
                        dictionary_id: 0,
                        synonym_group_ids: vec![],
                        cumulative_cost: None,
                    })
                    .collect(),
            })
        }
    }
    fn report(text: &str, pos: bool) -> RecognitionReport {
        let metadata = StatisticsMetadata {
            id: "fixture.statistics".into(),
            domain: "fixture".into(),
            corpus_id: "synthetic".into(),
            corpus_sha256: "a".repeat(64),
            license: "CC0-1.0".into(),
            analyzer: Scalars.analyze("").unwrap().provenance,
        };
        let docs = vec![("aa".into(), Scalars.analyze("aa").unwrap())];
        let asset = if pos {
            StatisticsArtifact::fit_with_pos(metadata, &docs)
        } else {
            StatisticsArtifact::fit(metadata, &docs)
        }
        .unwrap();
        let engine =
            RecognitionEngine::new(std::sync::Arc::new(Scalars), RecognitionConfig::default())
                .unwrap()
                .with_statistics(LightweightStatistics::new(asset).unwrap().into());
        let mut input = RecognitionInput::new(text, RecognitionSource::Asr, "doc", "one");
        input.domain = Some("fixture");
        engine.evaluate_recognition(input).unwrap()
    }
    #[test]
    fn observed_partial_legacy_and_empty_features_remain_distinct() {
        assert_eq!(probes(&report("a1", true)).unwrap(), [Some(true); 5]);
        assert_eq!(probes(&report("aa", true)).unwrap(), [Some(false); 5]);
        assert_eq!(
            probes(&report("a?", true)).unwrap(),
            [Some(true), None, Some(true), None, None]
        );
        assert_eq!(
            probes(&report("a1", false)).unwrap(),
            [Some(true), None, Some(true), None, None]
        );
        assert_eq!(
            probes(&report("a", true)).unwrap(),
            [Some(false), None, None, None, None]
        );
        let mut legacy = report("aa", true);
        legacy.string_features = None;
        assert_eq!(
            probes(&legacy).unwrap(),
            [Some(false), Some(false), None, Some(false), None]
        );
    }
    #[test]
    fn offline_flags_never_replace_actual_decisions_or_trust_bad_reports() {
        let reference=json!({"id":"one","document_id":"doc","source":"asr","text":"a1","transcription":"aa","transcription_status":"verified"}).to_string();
        let observation = || Observation {
            schema_version: "kzn.recognition.observation.v1".into(),
            summary: json!({}),
            reports: vec![report("a1", true)],
        };
        let base = score(&reference, observation()).unwrap();
        let mut comparison = score_with_features(&reference, observation(), true).unwrap();
        assert_eq!(
            comparison["feature_ablation"]["probes"]["unseen_pos"]["flagged"],
            json!([0, 1, 0])
        );
        comparison
            .as_object_mut()
            .unwrap()
            .remove("feature_ablation");
        assert_eq!(base, comparison);
        assert_eq!(
            base["summary"]["decision_counts"]["undetermined"],
            json!([0, 1, 0])
        );
        let mut bad = observation();
        bad.reports[0]
            .string_features
            .as_mut()
            .unwrap()
            .scalar_count = 0;
        assert!(score_with_features(&reference, bad, true).is_err());
    }
}
