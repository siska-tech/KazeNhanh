//! Deterministic raw scalar features, not Unicode script detection or error scores.
use super::*;
use std::collections::BTreeMap;

const METHOD: &str = "raw_scalar_ranges.v1";
const MAX_TRANSITIONS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CharacterClass {
    AsciiLetter,
    AsciiDigit,
    AsciiWhitespace,
    HiraganaBlock,
    KatakanaBlock,
    HanBasicBlock,
    Other,
}
fn classify(c: char) -> CharacterClass {
    match c {
        'a'..='z' | 'A'..='Z' => CharacterClass::AsciiLetter,
        '0'..='9' => CharacterClass::AsciiDigit,
        '\t'..='\r' | ' ' => CharacterClass::AsciiWhitespace,
        '\u{3040}'..='\u{309f}' => CharacterClass::HiraganaBlock,
        '\u{30a0}'..='\u{30ff}' => CharacterClass::KatakanaBlock,
        '\u{4e00}'..='\u{9fff}' => CharacterClass::HanBasicBlock,
        _ => CharacterClass::Other,
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CharacterTransition {
    pub left: CharacterClass,
    pub right: CharacterClass,
    /// Encloses both adjacent Unicode scalars in the original UTF-8 text.
    pub span: ByteSpan,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StringObservation {
    pub method: String,
    pub scalar_count: usize,
    pub counts: BTreeMap<CharacterClass, usize>,
    pub transition_count: usize,
    pub transitions: Vec<CharacterTransition>,
    pub omitted_transitions: usize,
}
impl StringObservation {
    /// One pass; retains only the first 64 class changes. No normalization.
    pub fn observe(text: &str) -> Self {
        let mut result = Self {
            method: METHOD.into(),
            scalar_count: 0,
            counts: BTreeMap::new(),
            transition_count: 0,
            transitions: Vec::new(),
            omitted_transitions: 0,
        };
        let mut previous = None;
        for (start, c) in text.char_indices() {
            let class = classify(c);
            result.scalar_count += 1;
            *result.counts.entry(class).or_default() += 1;
            if let Some((left_start, left)) = previous {
                if left != class {
                    result.transition_count += 1;
                    if result.transitions.len() < MAX_TRANSITIONS {
                        result.transitions.push(CharacterTransition {
                            left,
                            right: class,
                            span: ByteSpan {
                                start: left_start,
                                end: start + c.len_utf8(),
                            },
                        });
                    } else {
                        result.omitted_transitions += 1;
                    }
                }
            }
            previous = Some((start, class));
        }
        result
    }
    /// Recompute from the raw text to reject forged counts, methods and positions.
    pub fn validate(&self, text: &str) -> Result<(), EvaluationError> {
        if *self != Self::observe(text) {
            return Err(EvaluationError::Contract(
                "string observation differs from raw text".into(),
            ));
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn raw_scalars_and_utf8_spans_are_preserved() {
        let text = "漢あカA1 🙂\u{301}";
        let row = StringObservation::observe(text);
        assert_eq!(row.scalar_count, 8);
        assert_eq!(row.transition_count, 6);
        assert_eq!(row.counts[&CharacterClass::Other], 2);
        assert_eq!(
            &text[row.transitions[0].span.start..row.transitions[0].span.end],
            "漢あ"
        );
        row.validate(text).unwrap();
        // Full-width/half-width forms, supplementary Han and combining marks stay Other.
        let other = StringObservation::observe("Ａ１ｶ𠮷\u{301}");
        assert_eq!(other.counts[&CharacterClass::Other], 5);
        assert_eq!(other.transition_count, 0);
    }
    #[test]
    fn empty_single_and_bounded_transitions() {
        assert_eq!(StringObservation::observe("").scalar_count, 0);
        assert_eq!(StringObservation::observe("漢").transition_count, 0);
        let row = StringObservation::observe(&"a1".repeat(100));
        assert_eq!(row.transition_count, 199);
        assert_eq!(row.transitions.len(), 64);
        assert_eq!(row.omitted_transitions, 135);
    }
    #[test]
    fn forged_observations_are_rejected() {
        let row = StringObservation::observe("漢1");
        let mut bad = row.clone();
        bad.scalar_count += 1;
        assert!(bad.validate("漢1").is_err());
        let mut bad = row.clone();
        bad.transitions[0].span.start = 1;
        assert!(bad.validate("漢1").is_err());
        let mut bad = row;
        bad.method = "unknown".into();
        assert!(bad.validate("漢1").is_err());
    }
}
