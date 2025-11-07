use std::collections::HashMap;

use crate::foundation::nlp_service::{split_sentences, NlpService, TokenizedMorpheme};

const DEFAULT_STOPWORDS: &[&str] = &[
    "する",
    "なる",
    "ある",
    "いる",
    "こと",
    "もの",
    "これ",
    "それ",
    "あれ",
    "ため",
    "よう",
    "そして",
    "しかし",
    "the",
    "a",
    "an",
    "and",
    "or",
    "of",
    "to",
    "for",
    "with",
    "this",
    "that",
    "from",
    "have",
    "has",
    "was",
    "were",
    "been",
    "is",
    "are",
    "it",
];

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SentenceNode {
    pub position: usize,
    pub text: String,
    pub tokens: Vec<String>,
    pub bag_of_words: HashMap<String, f32>,
}

impl SentenceNode {
    pub(crate) fn new(
        position: usize,
        text: String,
        tokens: Vec<String>,
        bag_of_words: HashMap<String, f32>,
    ) -> Self {
        Self {
            position,
            text,
            tokens,
            bag_of_words,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SentencePreprocessorOptions {
    min_characters: usize,
    max_token_frequency: usize,
    stopwords: &'static [&'static str],
}

impl SentencePreprocessorOptions {
    pub(crate) fn new(min_characters: usize, max_token_frequency: usize) -> Self {
        Self {
            min_characters,
            max_token_frequency,
            stopwords: DEFAULT_STOPWORDS,
        }
    }

    fn is_stopword(&self, token: &str) -> bool {
        self.stopwords.iter().any(|candidate| candidate == &token)
    }
}

pub(crate) trait SentenceTokenizer: Send + Sync {
    fn tokenize(&self, text: &str) -> Result<Vec<TokenizedMorpheme>, sudachi::error::SudachiError>;
}

impl SentenceTokenizer for NlpService {
    fn tokenize(&self, text: &str) -> Result<Vec<TokenizedMorpheme>, sudachi::error::SudachiError> {
        NlpService::tokenize(self, text)
    }
}

pub(crate) fn build_sentence_nodes(
    tokenizer: &dyn SentenceTokenizer,
    text: &str,
    options: &SentencePreprocessorOptions,
) -> Result<Vec<SentenceNode>, sudachi::error::SudachiError> {
    let sentences = split_sentences(text);

    let mut nodes = Vec::new();

    for (position, sentence) in sentences.into_iter().enumerate() {
        let trimmed = sentence.trim();
        if trimmed.chars().count() < options.min_characters {
            continue;
        }

        let morphemes = tokenizer.tokenize(trimmed)?;

        let mut tokens = Vec::new();
        let mut bag_of_words: HashMap<String, f32> = HashMap::new();

        for morpheme in morphemes.iter() {
            if let Some(token) = normalize_token(morpheme) {
                if options.is_stopword(&token) {
                    continue;
                }

                tokens.push(token.clone());
                let entry = bag_of_words.entry(token).or_insert(0.0);
                *entry += 1.0;
            }
        }

        if tokens.is_empty() {
            continue;
        }

        if options.max_token_frequency > 0 {
            for value in bag_of_words.values_mut() {
                if *value > options.max_token_frequency as f32 {
                    *value = options.max_token_frequency as f32;
                }
            }
        }

        nodes.push(SentenceNode::new(
            position,
            trimmed.to_string(),
            tokens,
            bag_of_words,
        ));
    }

    Ok(nodes)
}

fn normalize_token(morpheme: &TokenizedMorpheme) -> Option<String> {
    if !is_content_word(morpheme) {
        return None;
    }

    let mut candidate = preferred_form(morpheme)?;

    candidate = candidate
        .trim_matches(|c: char| {
            matches!(
                c,
                '\'' | '"'
                    | '`'
                    | '“'
                    | '”'
                    | '、'
                    | '。'
                    | ','
                    | '.'
                    | ':'
                    | ';'
                    | '('
                    | ')'
                    | '['
                    | ']'
                    | '{'
                    | '}'
                    | '!'
                    | '?'
            )
        })
        .to_string();

    if candidate.is_empty() {
        return None;
    }

    if candidate.is_ascii() {
        candidate.make_ascii_lowercase();
    }

    if candidate.len() == 1 && candidate.chars().all(|ch| ch.is_ascii_alphabetic()) {
        return None;
    }

    Some(candidate)
}

fn preferred_form(morpheme: &TokenizedMorpheme) -> Option<String> {
    let normalized = morpheme.normalized_form().trim();
    if !normalized.is_empty() && normalized != "*" {
        return Some(normalized.to_string());
    }

    let dictionary = morpheme.dictionary_form().trim();
    if !dictionary.is_empty() && dictionary != "*" {
        return Some(dictionary.to_string());
    }

    let surface = morpheme.surface().trim();
    if surface.is_empty() {
        None
    } else {
        Some(surface.to_string())
    }
}

fn is_content_word(morpheme: &TokenizedMorpheme) -> bool {
    let pos = morpheme.part_of_speech();
    if pos.is_empty() {
        return false;
    }

    matches!(pos[0].as_str(), "名詞" | "動詞" | "形容詞" | "副詞")
        || matches!(pos[0].as_str(), "カスタム" | "固有名詞")
        || morpheme
            .surface()
            .chars()
            .any(|ch| ch.is_ascii_alphabetic())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{build_sentence_nodes, SentencePreprocessorOptions, SentenceTokenizer};
    use crate::foundation::nlp_service::TokenizedMorpheme;
    use sudachi::error::SudachiError;

    #[derive(Default)]
    struct StubTokenizer {
        responses: HashMap<String, Vec<TokenizedMorpheme>>,
    }

    impl StubTokenizer {
        fn with_response(mut self, key: &str, value: Vec<TokenizedMorpheme>) -> Self {
            self.responses.insert(key.to_string(), value);
            self
        }
    }

    impl SentenceTokenizer for StubTokenizer {
        fn tokenize(&self, text: &str) -> Result<Vec<TokenizedMorpheme>, SudachiError> {
            Ok(self.responses.get(text).cloned().unwrap_or_else(Vec::new))
        }
    }

    fn morpheme(surface: &str, normalized: &str, pos0: &str) -> TokenizedMorpheme {
        TokenizedMorpheme::new(
            surface.to_string(),
            normalized.to_string(),
            normalized.to_string(),
            surface.to_string(),
            vec![pos0.to_string()],
            0,
            false,
            0,
            surface.len(),
            0,
            surface.len(),
            Vec::new(),
            1,
        )
    }

    #[test]
    fn filters_short_sentences_and_stopwords() {
        let tokenizer = StubTokenizer::default()
            .with_response(
                "要約対象の文章は十分な長さを持ちます。",
                vec![
                    morpheme("要約", "要約", "名詞"),
                    morpheme("対象", "対象", "名詞"),
                    morpheme("文章", "文章", "名詞"),
                ],
            )
            .with_response(
                "これは二文目で詳細を述べています。",
                vec![
                    morpheme("これ", "これ", "名詞"),
                    morpheme("二文", "二文", "名詞"),
                    morpheme("詳細", "詳細", "名詞"),
                ],
            );
        let options = SentencePreprocessorOptions::new(10, 3);

        let text = "要約対象の文章は十分な長さを持ちます。これは二文目で詳細を述べています。";
        let nodes = build_sentence_nodes(&tokenizer, text, &options).expect("tokens");

        assert!(!nodes.is_empty());
        assert!(nodes.iter().all(|node| node.text.chars().count() >= 10));
        assert!(nodes
            .iter()
            .flat_map(|node| node.tokens.iter())
            .all(|token| token != "する"));
    }

    #[test]
    fn clamps_token_frequencies() {
        let tokenizer = StubTokenizer::default().with_response(
            "テストテストテストテスト。",
            vec![
                morpheme("テスト", "テスト", "名詞"),
                morpheme("テスト", "テスト", "名詞"),
                morpheme("テスト", "テスト", "名詞"),
                morpheme("テスト", "テスト", "名詞"),
            ],
        );
        let options = SentencePreprocessorOptions::new(2, 1);

        let text = "テストテストテストテスト。";
        let nodes = build_sentence_nodes(&tokenizer, text, &options).expect("tokens");

        assert_eq!(nodes.len(), 1);
        let node = &nodes[0];
        assert!(node
            .bag_of_words
            .values()
            .all(|value| (*value - 1.0).abs() < f32::EPSILON));
    }

    #[test]
    fn skips_sentences_without_tokens() {
        let tokenizer = StubTokenizer::default();
        let options = SentencePreprocessorOptions::new(2, 2);

        let text = "!!!";
        let nodes = build_sentence_nodes(&tokenizer, text, &options).expect("tokens");

        assert!(nodes.is_empty());
    }
}
