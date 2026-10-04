//! Adjacent full-POS-vector pairs; absent POS never bridges across a token.
use super::*;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PosStatisticsArtifact {
    pub pair_count: u64,
    pub missing_pair_count: u64,
    /// Canonical JSON arrays of two complete backend POS vectors.
    pub pairs: BTreeMap<String, u64>,
}
fn usable(pos: &[String]) -> bool {
    !pos.is_empty()
        && pos.len() <= 16
        && pos[0] != "*"
        && pos.iter().all(|p| !p.trim().is_empty() && p.len() <= 128)
}
fn key(pair: &[Morpheme]) -> Option<String> {
    let left = &pair[0].part_of_speech;
    let right = &pair[1].part_of_speech;
    (usable(left) && usable(right))
        .then(|| serde_json::to_string(&[left, right]).expect("POS strings serialize"))
}
impl PosStatisticsArtifact {
    pub(super) fn fit(documents: &[(String, MorphAnalysis)]) -> Result<Self, EvaluationError> {
        let mut result = Self::default();
        for (_, analysis) in documents {
            for pair in analysis.morphemes.windows(2) {
                if let Some(key) = key(pair) {
                    increment(&mut result.pairs, key)?;
                    result.pair_count += 1;
                    if result.pairs.len() > MAX_ENTRIES {
                        return Err(invalid("POS entry limit exceeded"));
                    }
                } else {
                    result.missing_pair_count += 1;
                }
            }
        }
        Ok(result)
    }
    pub(super) fn validate(&self, word_pair_count: u64) -> Result<(), EvaluationError> {
        if self.pair_count.checked_add(self.missing_pair_count) != Some(word_pair_count) {
            return Err(invalid("POS coverage mismatch"));
        }
        let mut sum = 0u64;
        for (k, count) in &self.pairs {
            if k.len() > MAX_KEY_BYTES || *count == 0 {
                return Err(invalid("invalid POS key/count"));
            }
            let pair: [Vec<String>; 2] =
                serde_json::from_str(k).map_err(|_| invalid("invalid POS pair"))?;
            if !pair.iter().all(|p| usable(p))
                || serde_json::to_string(&pair).expect("POS serializes") != *k
            {
                return Err(invalid("noncanonical/missing POS key"));
            }
            sum = sum
                .checked_add(*count)
                .ok_or_else(|| invalid("POS count overflow"))?;
        }
        if sum != self.pair_count {
            return Err(invalid("POS total mismatch"));
        }
        Ok(())
    }
    pub(super) fn observe(&self, text: &str, analysis: &MorphAnalysis) -> PosPairObservation {
        let mut result = PosPairObservation {
            method: "adjacent_full_pos_vectors.v1".into(),
            corpus_pair_count: self.pair_count,
            available_pair_count: 0,
            missing_pair_count: 0,
            frequencies: (self.pair_count > 0).then(FrequencyObservation::default),
        };
        for pair in analysis.morphemes.windows(2) {
            if let Some(key) = key(pair) {
                result.available_pair_count += 1;
                if let Some(frequencies) = &mut result.frequencies {
                    frequencies.add(
                        ByteSpan::new(text, pair[0].span.start(), pair[1].span.end())
                            .expect("validated token spans"),
                        *self.pairs.get(&key).unwrap_or(&0),
                    );
                }
            } else {
                result.missing_pair_count += 1;
            }
        }
        result
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PosPairObservation {
    pub method: String,
    pub corpus_pair_count: u64,
    pub available_pair_count: usize,
    pub missing_pair_count: usize,
    /// None when the training corpus has no usable POS pairs; this is not 100% unseen.
    pub frequencies: Option<FrequencyObservation>,
}
impl PosPairObservation {
    pub(super) fn validate(&self, word_pairs: usize) -> Result<(), EvaluationError> {
        if self.method != "adjacent_full_pos_vectors.v1"
            || self
                .available_pair_count
                .checked_add(self.missing_pair_count)
                != Some(word_pairs)
            || self.frequencies.is_some() != (self.corpus_pair_count > 0)
            || self
                .frequencies
                .as_ref()
                .is_some_and(|f| f.unit_count != self.available_pair_count)
        {
            return Err(EvaluationError::Contract(
                "invalid POS observation method/coverage/missingness".into(),
            ));
        }
        Ok(())
    }
}
