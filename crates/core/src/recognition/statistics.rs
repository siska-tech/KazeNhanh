//! Sparse, corpus-relative observations. Frequencies are never error probabilities.
use super::*;
use std::collections::BTreeMap;

mod pos;
pub use pos::*;
pub const STATISTICS_POS_SCHEMA: &str = "kzn.statistics.v2";

pub const STATISTICS_SCHEMA: &str = "kzn.statistics.v1";
const MAX_ENTRIES: usize = 100_000;
const MAX_KEY_BYTES: usize = 4096;
const MAX_DOCUMENT_BYTES: usize = 65_536;
const MAX_EVENTS: usize = 64;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatisticsMetadata {
    pub id: String,
    pub domain: String,
    pub corpus_id: String,
    pub corpus_sha256: String,
    pub license: String,
    /// Exact analyzer/dictionary/settings identities, not just the dictionary name.
    pub analyzer: Vec<ArtifactIdentity>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatisticsArtifact {
    pub schema_version: String,
    pub metadata: StatisticsMetadata,
    pub document_count: u64,
    pub character_pair_count: u64,
    pub word_count: u64,
    pub word_pair_count: u64,
    pub character_pairs: BTreeMap<String, u64>,
    pub words: BTreeMap<String, u64>,
    /// Keys are canonical JSON arrays of two dictionary-form strings.
    pub word_pairs: BTreeMap<String, u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos: Option<PosStatisticsArtifact>,
}
fn invalid(reason: &str) -> EvaluationError {
    EvaluationError::InvalidConfig(format!("statistics artifact: {reason}"))
}
fn hash_valid(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn pair_key(left: &str, right: &str) -> String {
    serde_json::to_string(&[left, right]).expect("strings serialize")
}
fn word(token: &Morpheme) -> &str {
    if token.dictionary_form.is_empty() {
        &token.surface
    } else {
        &token.dictionary_form
    }
}
impl StatisticsMetadata {
    fn validate(&self) -> Result<(), EvaluationError> {
        if [&self.id, &self.domain, &self.corpus_id, &self.license]
            .iter()
            .any(|s| s.trim().is_empty() || s.len() > MAX_KEY_BYTES)
            || !hash_valid(&self.corpus_sha256)
            || self.analyzer.is_empty()
            || self.analyzer.len() > 16
        {
            return Err(invalid(
                "required provenance/domain/license/hash missing or oversized",
            ));
        }
        let mut components = std::collections::HashSet::new();
        for artifact in &self.analyzer {
            if artifact.component.trim().is_empty()
                || artifact.id.trim().is_empty()
                || artifact.component.len() > MAX_KEY_BYTES
                || artifact.id.len() > MAX_KEY_BYTES
                || !components.insert(&artifact.component)
                || artifact.sha256.as_ref().is_some_and(|s| !hash_valid(s))
            {
                return Err(invalid("invalid or duplicate analyzer identity"));
            }
        }
        Ok(())
    }
}
impl StatisticsArtifact {
    pub fn validate(&self) -> Result<(), EvaluationError> {
        self.metadata.validate()?;
        if self.document_count == 0
            || !matches!(
                (self.schema_version.as_str(), self.pos.is_some()),
                (STATISTICS_SCHEMA, false) | (STATISTICS_POS_SCHEMA, true)
            )
        {
            return Err(invalid("unsupported schema or empty corpus"));
        }
        if self.character_pairs.len()
            + self.words.len()
            + self.word_pairs.len()
            + self.pos.as_ref().map_or(0, |pos| pos.pairs.len())
            > MAX_ENTRIES
        {
            return Err(invalid("entry limit exceeded"));
        }
        for (table, total) in [
            (&self.character_pairs, self.character_pair_count),
            (&self.words, self.word_count),
            (&self.word_pairs, self.word_pair_count),
        ] {
            let mut sum = 0u64;
            for (key, count) in table {
                if key.is_empty() || key.len() > MAX_KEY_BYTES || *count == 0 {
                    return Err(invalid("empty/oversized key or zero count"));
                }
                sum = sum
                    .checked_add(*count)
                    .ok_or_else(|| invalid("count overflow"))?;
            }
            if sum != total {
                return Err(invalid("count totals disagree"));
            }
        }
        if self.character_pairs.keys().any(|k| k.chars().count() != 2)
            || self.word_pair_count > self.word_count
        {
            return Err(invalid("invalid character/word pair"));
        }
        if let Some(pos) = &self.pos {
            pos.validate(self.word_pair_count)?;
        }
        let mut outgoing: BTreeMap<&str, u64> = BTreeMap::new();
        let mut incoming: BTreeMap<&str, u64> = BTreeMap::new();
        for (key, count) in &self.word_pairs {
            let pair: [String; 2] =
                serde_json::from_str(key).map_err(|_| invalid("invalid pair key"))?;
            if pair_key(&pair[0], &pair[1]) != *key
                || pair.iter().any(|w| !self.words.contains_key(w))
            {
                return Err(invalid("noncanonical pair or absent word"));
            }
            let left = self
                .words
                .get_key_value(&pair[0])
                .expect("checked word")
                .0
                .as_str();
            let right = self
                .words
                .get_key_value(&pair[1])
                .expect("checked word")
                .0
                .as_str();
            *outgoing.entry(left).or_default() += count;
            *incoming.entry(right).or_default() += count;
        }
        for (word, count) in outgoing.into_iter().chain(incoming) {
            if count > self.words[word] {
                return Err(invalid("pair frequency exceeds word frequency"));
            }
        }
        Ok(())
    }
    /// Fit version 2 with POS pairs while preserving the legacy version-1 fitting API.
    pub fn fit_with_pos(
        metadata: StatisticsMetadata,
        documents: &[(String, MorphAnalysis)],
    ) -> Result<Self, EvaluationError> {
        let mut asset = Self::fit(metadata, documents)?;
        asset.pos = Some(PosStatisticsArtifact::fit(documents)?);
        asset.schema_version = STATISTICS_POS_SCHEMA.into();
        asset.validate()?;
        Ok(asset)
    }
    /// Training utility: caller supplies clean texts separately from evaluation inputs.
    /// Neither gold labels nor recognition confidence participate in fitting.
    pub fn fit(
        metadata: StatisticsMetadata,
        documents: &[(String, MorphAnalysis)],
    ) -> Result<Self, EvaluationError> {
        metadata.validate()?;
        if documents.len() > 10_000 {
            return Err(invalid("document limit exceeded"));
        }
        let mut asset = Self {
            schema_version: STATISTICS_SCHEMA.into(),
            metadata,
            document_count: 0,
            character_pair_count: 0,
            word_count: 0,
            word_pair_count: 0,
            character_pairs: BTreeMap::new(),
            words: BTreeMap::new(),
            word_pairs: BTreeMap::new(),
            pos: None,
        };
        for (text, analysis) in documents {
            if text.len() > MAX_DOCUMENT_BYTES || analysis.provenance != asset.metadata.analyzer {
                return Err(invalid("document oversized or analyzer mismatch"));
            }
            validate_morphology(text, analysis)?;
            asset.document_count += 1;
            let chars: Vec<_> = text.chars().collect();
            for pair in chars.windows(2) {
                increment(&mut asset.character_pairs, pair.iter().collect())?;
                asset.character_pair_count += 1;
            }
            for token in &analysis.morphemes {
                increment(&mut asset.words, word(token).into())?;
                asset.word_count += 1;
            }
            for pair in analysis.morphemes.windows(2) {
                increment(
                    &mut asset.word_pairs,
                    pair_key(word(&pair[0]), word(&pair[1])),
                )?;
                asset.word_pair_count += 1;
            }
            if asset.character_pairs.len() + asset.words.len() + asset.word_pairs.len()
                > MAX_ENTRIES
            {
                return Err(invalid("entry limit exceeded"));
            }
        }
        asset.validate()?;
        Ok(asset)
    }
}
fn increment(table: &mut BTreeMap<String, u64>, key: String) -> Result<(), EvaluationError> {
    if key.is_empty() || key.len() > MAX_KEY_BYTES {
        return Err(invalid("invalid key length"));
    }
    let count = table.entry(key).or_default();
    *count = count
        .checked_add(1)
        .ok_or_else(|| invalid("count overflow"))?;
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatisticsEvent {
    pub span: ByteSpan,
    pub corpus_count: u64,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrequencyObservation {
    pub unit_count: usize,
    pub unseen_count: usize,
    /// None when there are no units; zero is not missing.
    pub unseen_fraction: Option<f64>,
    /// First unseen events, bounded independently for each family.
    pub unseen_events: Vec<StatisticsEvent>,
    pub omitted_event_count: usize,
}
impl FrequencyObservation {
    fn add(&mut self, span: ByteSpan, count: u64) {
        self.unit_count += 1;
        if count == 0 {
            self.unseen_count += 1;
            if self.unseen_events.len() < MAX_EVENTS {
                self.unseen_events.push(StatisticsEvent {
                    span,
                    corpus_count: count,
                });
            } else {
                self.omitted_event_count += 1;
            }
        }
        self.unseen_fraction = Some(self.unseen_count as f64 / self.unit_count as f64);
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatisticsObservation {
    pub method: String,
    pub asset_id: String,
    pub corpus_id: String,
    pub corpus_sha256: String,
    pub corpus_document_count: u64,
    pub domain: String,
    pub character_pairs: FrequencyObservation,
    pub words: FrequencyObservation,
    pub word_pairs: FrequencyObservation,
    pub oov_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos: Option<PosPairObservation>,
}
impl StatisticsObservation {
    pub fn validate(
        &self,
        text: &str,
        domain: Option<&str>,
        provider_id: &str,
    ) -> Result<(), EvaluationError> {
        if self.method != "raw_unicode_scalar_bigrams_dictionary_form_unigrams_bigrams.v1"
            || self.asset_id != provider_id
            || self.asset_id.trim().is_empty()
            || domain != Some(self.domain.as_str())
            || self.corpus_id.trim().is_empty()
            || !hash_valid(&self.corpus_sha256)
            || self.corpus_document_count == 0
            || self.character_pairs.unit_count != text.chars().count().saturating_sub(1)
            || self.words.unit_count > text.chars().count()
            || self.word_pairs.unit_count != self.words.unit_count.saturating_sub(1)
            || self.oov_count > self.words.unit_count
        {
            return Err(EvaluationError::Contract(
                "invalid statistics observation identity/coverage".into(),
            ));
        }
        if let Some(pos) = &self.pos {
            pos.validate(self.word_pairs.unit_count)?;
        }
        for family in [&self.character_pairs, &self.words, &self.word_pairs]
            .into_iter()
            .chain(self.pos.as_ref().and_then(|p| p.frequencies.as_ref()))
        {
            let expected = if family.unit_count == 0 {
                None
            } else {
                Some(family.unseen_count as f64 / family.unit_count as f64)
            };
            if family.unseen_count > family.unit_count
                || family.unseen_fraction != expected
                || family.unseen_events.len() != family.unseen_count.min(MAX_EVENTS)
                || family.omitted_event_count != family.unseen_count.saturating_sub(MAX_EVENTS)
            {
                return Err(EvaluationError::Contract(
                    "invalid statistics counts/fraction/bounds".into(),
                ));
            }
            let mut previous = None;
            for event in &family.unseen_events {
                event.span.validate(text)?;
                if event.corpus_count != 0
                    || event.span.start() == event.span.end()
                    || previous.is_some_and(|start| event.span.start() <= start)
                {
                    return Err(EvaluationError::Contract(
                        "invalid unseen statistics event".into(),
                    ));
                }
                previous = Some(event.span.start());
            }
        }
        for event in &self.character_pairs.unseen_events {
            if text[event.span.start()..event.span.end()].chars().count() != 2 {
                return Err(EvaluationError::Contract(
                    "invalid character pair span".into(),
                ));
            }
        }
        Ok(())
    }
}
/// Immutable validated sparse tables. No neural runtime, fusion, or decision threshold.
pub struct LightweightStatistics {
    artifact: StatisticsArtifact,
}
impl LightweightStatistics {
    pub fn new(artifact: StatisticsArtifact) -> Result<Self, EvaluationError> {
        artifact.validate()?;
        Ok(Self { artifact })
    }
    pub fn artifact_id(&self) -> &str {
        &self.artifact.metadata.id
    }
    pub(crate) fn observe(
        &self,
        text: &str,
        domain: Option<&str>,
        analysis: &MorphAnalysis,
    ) -> Result<StatisticsObservation, &'static str> {
        if domain != Some(self.artifact.metadata.domain.as_str()) {
            return Err("statistics_domain_missing_or_mismatch");
        }
        if analysis.provenance != self.artifact.metadata.analyzer {
            return Err("statistics_analyzer_artifacts_mismatch");
        }
        if text.len() > MAX_DOCUMENT_BYTES {
            return Err("statistics_input_limit_exceeded");
        }
        let mut output = StatisticsObservation {
            method: "raw_unicode_scalar_bigrams_dictionary_form_unigrams_bigrams.v1".into(),
            asset_id: self.artifact.metadata.id.clone(),
            corpus_id: self.artifact.metadata.corpus_id.clone(),
            corpus_sha256: self.artifact.metadata.corpus_sha256.clone(),
            corpus_document_count: self.artifact.document_count,
            domain: self.artifact.metadata.domain.clone(),
            character_pairs: FrequencyObservation::default(),
            words: FrequencyObservation::default(),
            word_pairs: FrequencyObservation::default(),
            oov_count: analysis.morphemes.iter().filter(|w| w.is_oov).count(),
            pos: self
                .artifact
                .pos
                .as_ref()
                .map(|p| p.observe(text, analysis)),
        };
        let chars: Vec<_> = text.char_indices().collect();
        for pair in chars.windows(2) {
            let key: String = pair.iter().map(|(_, c)| c).collect();
            output.character_pairs.add(
                ByteSpan::new(text, pair[0].0, pair[1].0 + pair[1].1.len_utf8())
                    .expect("character boundaries"),
                *self.artifact.character_pairs.get(&key).unwrap_or(&0),
            );
        }
        for token in &analysis.morphemes {
            output.words.add(
                token.span,
                *self.artifact.words.get(word(token)).unwrap_or(&0),
            );
        }
        for pair in analysis.morphemes.windows(2) {
            output.word_pairs.add(
                ByteSpan::new(text, pair[0].span.start(), pair[1].span.end())
                    .expect("ordered morphemes"),
                *self
                    .artifact
                    .word_pairs
                    .get(&pair_key(word(&pair[0]), word(&pair[1])))
                    .unwrap_or(&0),
            );
        }
        Ok(output)
    }
}

#[cfg(test)]
mod tests;
