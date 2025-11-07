use crate::EngineConfig;
use saku::SentenceTokenizer;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
use sudachi::analysis::morpheme::Morpheme as SudachiMorpheme;
use sudachi::analysis::stateless_tokenizer::StatelessTokenizer;
use sudachi::analysis::Mode;
use sudachi::analysis::Tokenize;
use sudachi::config::ConfigBuilder;
use sudachi::dic::dictionary::JapaneseDictionary;
use sudachi::dic::storage::{Storage, SudachiDicData};
use sudachi::error::SudachiError;

#[derive(Clone)]
pub struct NlpService {
    tokenizer: Arc<StatelessTokenizer<Arc<JapaneseDictionary>>>,
}

impl NlpService {
    pub fn new(config: &EngineConfig) -> Result<Self, SudachiError> {
        let builder = ConfigBuilder::from_bytes(config.settings_bytes)?;
        let sudachi_config = builder.build();

        let storage = SudachiDicData::new(Storage::Borrowed(config.dictionary_bytes));
        let dictionary = match catch_unwind(AssertUnwindSafe(|| {
            JapaneseDictionary::from_cfg_storage_with_embedded_chardef(&sudachi_config, storage)
        })) {
            Ok(result) => result?,
            Err(_) => return Err(SudachiError::InvalidDictionaryGrammar),
        };

        let tokenizer = StatelessTokenizer::new(Arc::new(dictionary));

        Ok(Self {
            tokenizer: Arc::new(tokenizer),
        })
    }

    pub fn tokenize(&self, text: &str) -> Result<Vec<TokenizedMorpheme>, SudachiError> {
        let morpheme_list = self.tokenizer.tokenize(text, Mode::C, false)?;

        let mut results = Vec::with_capacity(morpheme_list.len());
        for morpheme in morpheme_list.iter() {
            results.push(TokenizedMorpheme::from_morpheme(&morpheme));
        }

        Ok(results)
    }
}

pub fn split_sentences(text: &str) -> Vec<String> {
    if text.trim().is_empty() {
        return Vec::new();
    }

    let tokenizer = SentenceTokenizer::default();

    tokenizer
        .tokenize_raw(text)
        .into_iter()
        .flat_map(explode_sentence)
        .collect()
}

fn explode_sentence(segment: &str) -> Vec<String> {
    const TERMINATORS: [char; 8] = ['。', '．', '.', '｡', '！', '!', '？', '?'];
    const CLOSERS: [char; 6] = ['」', '』', '）', ')', '】', '〉'];

    let mut results: Vec<String> = Vec::new();
    let mut buffer = String::new();
    let mut pending_closure = false;

    for ch in segment.chars() {
        if pending_closure {
            if ch.is_whitespace() {
                continue;
            }

            if CLOSERS.contains(&ch) {
                if let Some(last) = results.last_mut() {
                    last.push(ch);
                    continue;
                }
            }

            pending_closure = false;
        }

        buffer.push(ch);
        if TERMINATORS.contains(&ch) {
            push_buffer(&mut results, &mut buffer);
            pending_closure = true;
        }
    }

    push_buffer(&mut results, &mut buffer);

    results
}

fn push_buffer(results: &mut Vec<String>, buffer: &mut String) {
    let trimmed = buffer.trim();
    if trimmed.is_empty() {
        buffer.clear();
        return;
    }

    let trimmed_owned = trimmed.to_string();
    let is_only_punct = trimmed_owned
        .chars()
        .all(|ch| matches!(ch, '!' | '?' | '！' | '？'));

    if is_only_punct {
        if let Some(last) = results.last_mut() {
            last.push_str(&trimmed_owned);
        } else {
            results.push(trimmed_owned);
        }
    } else {
        results.push(trimmed_owned);
    }

    buffer.clear();
}

#[derive(Clone, Debug, PartialEq)]
pub struct TokenizedMorpheme {
    surface: String,
    dictionary_form: String,
    normalized_form: String,
    reading_form: String,
    part_of_speech: Vec<String>,
    dictionary_id: i32,
    is_oov: bool,
    begin: usize,
    end: usize,
    begin_char: usize,
    end_char: usize,
    synonym_group_ids: Vec<u32>,
    total_cost: i32,
}

impl TokenizedMorpheme {
    pub fn new(
        surface: String,
        dictionary_form: String,
        normalized_form: String,
        reading_form: String,
        part_of_speech: Vec<String>,
        dictionary_id: i32,
        is_oov: bool,
        begin: usize,
        end: usize,
        begin_char: usize,
        end_char: usize,
        synonym_group_ids: Vec<u32>,
        total_cost: i32,
    ) -> Self {
        Self {
            surface,
            dictionary_form,
            normalized_form,
            reading_form,
            part_of_speech,
            dictionary_id,
            is_oov,
            begin,
            end,
            begin_char,
            end_char,
            synonym_group_ids,
            total_cost,
        }
    }

    pub fn from_morpheme(morpheme: &SudachiMorpheme<'_, Arc<JapaneseDictionary>>) -> Self {
        let surface = morpheme.surface().to_string();
        let dictionary_form = morpheme.dictionary_form().to_string();
        let normalized_form = morpheme.normalized_form().to_string();
        let reading_form = morpheme.reading_form().to_string();
        let part_of_speech = morpheme.part_of_speech().to_vec();
        let dictionary_id = morpheme.dictionary_id();
        let is_oov = morpheme.is_oov();
        let begin = morpheme.begin();
        let end = morpheme.end();
        let begin_char = morpheme.begin_c();
        let end_char = morpheme.end_c();
        let synonym_group_ids = morpheme.synonym_group_ids().to_vec();
        let total_cost = morpheme.total_cost();

        Self::new(
            surface,
            dictionary_form,
            normalized_form,
            reading_form,
            part_of_speech,
            dictionary_id,
            is_oov,
            begin,
            end,
            begin_char,
            end_char,
            synonym_group_ids,
            total_cost,
        )
    }

    pub fn surface(&self) -> &str {
        &self.surface
    }

    pub fn dictionary_form(&self) -> &str {
        &self.dictionary_form
    }

    pub fn normalized_form(&self) -> &str {
        &self.normalized_form
    }

    pub fn reading_form(&self) -> &str {
        &self.reading_form
    }

    pub fn part_of_speech(&self) -> &[String] {
        &self.part_of_speech
    }

    pub fn dictionary_id(&self) -> i32 {
        self.dictionary_id
    }

    pub fn is_oov(&self) -> bool {
        self.is_oov
    }

    pub fn begin(&self) -> usize {
        self.begin
    }

    pub fn end(&self) -> usize {
        self.end
    }

    pub fn begin_c(&self) -> usize {
        self.begin_char
    }

    pub fn end_c(&self) -> usize {
        self.end_char
    }

    pub fn synonym_group_ids(&self) -> &[u32] {
        &self.synonym_group_ids
    }

    pub fn total_cost(&self) -> i32 {
        self.total_cost
    }
}

#[cfg(test)]
mod tests {
    use super::{split_sentences, NlpService, TokenizedMorpheme};
    use crate::EngineConfig;

    #[test]
    fn initialization_fails_with_invalid_dictionary_bytes() {
        let config = EngineConfig::new(b"model", b"invalid", br#"{}"#);

        let result = NlpService::new(&config);

        assert!(result.is_err());
    }

    #[test]
    fn tokenized_morpheme_accessors_return_expected_values() {
        let morpheme = TokenizedMorpheme::new(
            "cat".to_string(),
            "cat".to_string(),
            "cat".to_string(),
            "cat".to_string(),
            vec!["noun".to_string(), "common".to_string()],
            0,
            false,
            0,
            3,
            0,
            2,
            vec![1, 2],
            42,
        );

        assert_eq!(morpheme.surface(), "cat");
        assert_eq!(morpheme.dictionary_form(), "cat");
        assert_eq!(morpheme.normalized_form(), "cat");
        assert_eq!(morpheme.reading_form(), "cat");
        assert_eq!(morpheme.part_of_speech()[0], "noun");
        assert_eq!(morpheme.dictionary_id(), 0);
        assert!(!morpheme.is_oov());
        assert_eq!(morpheme.begin(), 0);
        assert_eq!(morpheme.end(), 3);
        assert_eq!(morpheme.begin_c(), 0);
        assert_eq!(morpheme.end_c(), 2);
        assert_eq!(morpheme.synonym_group_ids(), &[1, 2]);
        assert_eq!(morpheme.total_cost(), 42);
    }

    #[test]
    fn split_sentences_returns_trimmed_segments() {
        let text = "風が吹けば桶屋が儲かる。雨が降る?\n\n次の文!";

        let sentences = split_sentences(text);

        assert_eq!(
            sentences,
            vec![
                "風が吹けば桶屋が儲かる。".to_string(),
                "雨が降る?".to_string(),
                "次の文!".to_string(),
            ]
        );
    }

    #[test]
    fn split_sentences_returns_empty_for_blank_input() {
        let sentences = split_sentences("   \n\n   ");

        assert!(sentences.is_empty());
    }

    #[test]
    fn split_sentences_merges_multi_punctuation() {
        let text = "本当？！\n信じられない!!";

        let sentences = split_sentences(text);

        assert_eq!(
            sentences,
            vec!["本当？！".to_string(), "信じられない!!".to_string(),]
        );
    }

    #[test]
    fn split_sentences_handles_ascii_periods_and_whitespace() {
        let text = " First sentence.   Second sentence.  ";

        let sentences = split_sentences(text);

        assert_eq!(
            sentences,
            vec![
                "First sentence.".to_string(),
                "Second sentence.".to_string(),
            ]
        );
    }

    #[test]
    fn split_sentences_preserves_japanese_quotes() {
        let text = "「こんにちは。」今日は晴れです。";

        let sentences = split_sentences(text);

        assert_eq!(
            sentences,
            vec![
                "「こんにちは。」".to_string(),
                "今日は晴れです。".to_string(),
            ]
        );
    }
}
