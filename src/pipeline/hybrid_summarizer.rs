use std::sync::{Arc, Mutex, MutexGuard};

use crate::{foundation::nlp_service::split_sentences, inference::InferenceEngine, KazeNhanhError};

use super::lex_rank_engine::{rank_sentences, LexRankOptions, RankedSentence};
use super::sentence::{
    build_sentence_nodes, SentenceNode, SentencePreprocessorOptions, SentenceTokenizer,
};

#[derive(Clone, Debug)]
pub(crate) struct HybridSummarizerConfig {
    pub max_sentences: usize,
    pub similarity_threshold: f32,
    pub damping_factor: f32,
    pub max_iterations: usize,
    pub convergence_delta: f32,
    pub min_characters: usize,
    pub max_token_frequency: usize,
    #[allow(dead_code)]
    pub retry_on_failure: bool,
}

impl Default for HybridSummarizerConfig {
    fn default() -> Self {
        Self {
            max_sentences: 5,
            similarity_threshold: 0.12,
            damping_factor: 0.85,
            max_iterations: 50,
            convergence_delta: 1e-4,
            min_characters: 25,
            max_token_frequency: 6,
            retry_on_failure: false,
        }
    }
}

#[derive(Clone)]
pub(crate) struct HybridSummarizer {
    tokenizer: Arc<dyn SentenceTokenizer + Send + Sync>,
    inference_engine: Arc<Mutex<InferenceEngine>>,
    config: HybridSummarizerConfig,
}

impl HybridSummarizer {
    pub(crate) fn new(
        tokenizer: Arc<dyn SentenceTokenizer + Send + Sync>,
        inference_engine: Arc<Mutex<InferenceEngine>>,
        config: HybridSummarizerConfig,
    ) -> Self {
        Self {
            tokenizer,
            inference_engine,
            config,
        }
    }

    pub(crate) fn preprocessor_options(&self) -> SentencePreprocessorOptions {
        SentencePreprocessorOptions::new(
            self.config.min_characters,
            self.config.max_token_frequency,
        )
    }

    pub(crate) fn lex_rank_options(&self) -> LexRankOptions {
        LexRankOptions::from(&self.config)
    }

    pub(crate) fn prepare_sentences(
        &self,
        text: &str,
    ) -> Result<Vec<SentenceNode>, KazeNhanhError> {
        build_sentence_nodes(self.tokenizer.as_ref(), text, &self.preprocessor_options())
            .map_err(|source| KazeNhanhError::TokenizationError { source })
    }

    pub(crate) fn execute(&self, text: &str) -> Result<HybridSummary, KazeNhanhError> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err(KazeNhanhError::InvalidInput(
                "HybridSummarizer::execute received empty text".to_string(),
            ));
        }

        let sentence_nodes = self.prepare_sentences(trimmed)?;
        let options = self.lex_rank_options();

        let ranked = rank_sentences(&sentence_nodes, &options)
            .map_err(|error| KazeNhanhError::SummarizeEngineError(error.to_string()))?;

        let mut selected = select_sentences(
            ranked,
            self.config.max_sentences,
            options.similarity_threshold,
        );

        if selected.is_empty() {
            if let Some((position, sentence)) = fallback_sentence(trimmed) {
                selected.push(RankedSentence::new(position, 1.0, sentence));
            } else {
                return Err(KazeNhanhError::SummarizeEngineError(
                    "lexrank produced no candidates".to_string(),
                ));
            }
        }

        selected.sort_by(|a, b| a.position.cmp(&b.position));
        let extracted_sentences = selected
            .into_iter()
            .map(|candidate| candidate.text)
            .collect::<Vec<_>>();

        let prompt = build_synthesis_prompt(trimmed, &extracted_sentences);

        let mut engine = self.lock_inference()?;
        let synthesized = engine.synthesize(&prompt)?;

        Ok(HybridSummary::new(extracted_sentences, Some(synthesized)))
    }

    fn lock_inference(&self) -> Result<MutexGuard<'_, InferenceEngine>, KazeNhanhError> {
        self.inference_engine.lock().map_err(|_| {
            KazeNhanhError::SummarizeEngineError("failed to lock inference engine".to_string())
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HybridSummary {
    pub extracted_sentences: Vec<String>,
    pub synthesized_summary: Option<String>,
}

fn select_sentences(
    ranked: Vec<RankedSentence>,
    max_sentences: usize,
    score_threshold: f32,
) -> Vec<RankedSentence> {
    if max_sentences == 0 {
        return Vec::new();
    }

    let mut selected = Vec::new();

    for candidate in ranked {
        if candidate.score >= score_threshold || selected.is_empty() {
            selected.push(candidate);
        }

        if selected.len() == max_sentences {
            break;
        }
    }

    selected
}

fn fallback_sentence(text: &str) -> Option<(usize, String)> {
    split_sentences(text)
        .into_iter()
        .enumerate()
        .find_map(|(index, sentence)| {
            let trimmed = sentence.trim();
            if trimmed.is_empty() || is_noise_sentence(trimmed) {
                None
            } else {
                Some((index, trimmed.to_string()))
            }
        })
}

fn build_synthesis_prompt(original_text: &str, sentences: &[String]) -> String {
    let mut prompt = String::from(
        "以下の重要文をもとに90文字程度の要約を出力してください。文体は敬体でまとめてください。\n",
    );

    for (idx, sentence) in sentences.iter().enumerate() {
        prompt.push_str(&format!("{}. {}\n", idx + 1, sentence));
    }

    prompt.push_str("\n原文:\n");
    prompt.push_str(original_text);
    prompt
}

fn is_noise_sentence(sentence: &str) -> bool {
    sentence
        .chars()
        .all(|ch| matches!(ch, '!' | '?' | '！' | '？' | '。' | '、'))
}

impl HybridSummary {
    pub(crate) fn new(
        extracted_sentences: Vec<String>,
        synthesized_summary: Option<String>,
    ) -> Self {
        Self {
            extracted_sentences,
            synthesized_summary,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_synthesis_prompt, fallback_sentence, select_sentences, HybridSummarizer,
        HybridSummarizerConfig,
    };
    use crate::foundation::nlp_service::TokenizedMorpheme;
    use crate::EngineConfig;
    use crate::{
        inference::InferenceEngine, pipeline::lex_rank_engine::RankedSentence,
        pipeline::sentence::SentenceTokenizer,
    };
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

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

    #[derive(Default)]
    struct StubTokenizer {
        responses: HashMap<String, Vec<TokenizedMorpheme>>,
    }

    impl StubTokenizer {
        fn with_response(mut self, sentence: &str, tokens: Vec<TokenizedMorpheme>) -> Self {
            self.responses.insert(sentence.to_string(), tokens);
            self
        }
    }

    impl SentenceTokenizer for StubTokenizer {
        fn tokenize(
            &self,
            text: &str,
        ) -> Result<Vec<TokenizedMorpheme>, sudachi::error::SudachiError> {
            Ok(self.responses.get(text).cloned().unwrap_or_else(Vec::new))
        }
    }

    fn inference_engine() -> Arc<Mutex<InferenceEngine>> {
        let config = EngineConfig::new(b"model-bytes", b"dict", br#"{}"#);
        let engine = InferenceEngine::new(&config).expect("inference engine initializes");
        Arc::new(Mutex::new(engine))
    }

    #[test]
    fn executes_pipeline_and_returns_summary() {
        let tokenizer = StubTokenizer::default()
            .with_response(
                "Rustは安全性を重視する言語です。",
                vec![
                    morpheme("Rust", "rust", "名詞"),
                    morpheme("安全性", "安全性", "名詞"),
                    morpheme("言語", "言語", "名詞"),
                ],
            )
            .with_response(
                "並列処理でも高い信頼性を維持します。",
                vec![
                    morpheme("Rust", "rust", "名詞"),
                    morpheme("並列", "並列", "名詞"),
                    morpheme("処理", "処理", "名詞"),
                    morpheme("信頼性", "信頼性", "名詞"),
                ],
            );

        let mut config = HybridSummarizerConfig::default();
        config.similarity_threshold = 0.0;
        config.min_characters = 2;

        let summarizer = HybridSummarizer::new(
            Arc::new(tokenizer) as Arc<dyn SentenceTokenizer + Send + Sync>,
            inference_engine(),
            config,
        );

        let input = "Rustは安全性を重視する言語です。並列処理でも高い信頼性を維持します。";
        let summary = summarizer
            .execute(input)
            .expect("hybrid summarization succeeds");

        assert_eq!(summary.extracted_sentences.len(), 2);
        let synthesized = summary
            .synthesized_summary
            .as_ref()
            .expect("synthesized summary exists");
        assert!(synthesized.contains("Rustは安全性を重視する言語です。"));
    }

    #[test]
    fn propagates_lexrank_failure() {
        let tokenizer = StubTokenizer::default().with_response(
            "短い文です。",
            vec![
                morpheme("短い", "短い", "名詞"),
                morpheme("文", "文", "名詞"),
            ],
        );

        let mut config = HybridSummarizerConfig::default();
        config.similarity_threshold = 0.9;
        config.max_iterations = 0;
        config.min_characters = 2;

        let summarizer = HybridSummarizer::new(
            Arc::new(tokenizer) as Arc<dyn SentenceTokenizer + Send + Sync>,
            inference_engine(),
            config,
        );

        let error = summarizer
            .execute("短い文です。")
            .err()
            .expect("expected summarization error");

        match error {
            crate::KazeNhanhError::SummarizeEngineError(message) => {
                assert!(message.contains("lexrank failed to converge"));
            }
            other => panic!("unexpected error: {:?}", other),
        }
    }

    #[test]
    fn execute_falls_back_when_no_ranked_sentences() {
        let tokenizer = StubTokenizer::default();
        let config = HybridSummarizerConfig::default();

        let summarizer = HybridSummarizer::new(
            Arc::new(tokenizer) as Arc<dyn SentenceTokenizer + Send + Sync>,
            inference_engine(),
            config,
        );

        let summary = summarizer
            .execute("短い文です。")
            .expect("fallback should succeed");

        assert_eq!(
            summary.extracted_sentences,
            vec!["短い文です。".to_string()]
        );
        let synthesized = summary
            .synthesized_summary
            .as_ref()
            .expect("synthesized summary missing");
        assert!(synthesized.contains("短い文です。"));
    }

    #[test]
    fn select_sentences_respects_threshold_but_keeps_first() {
        let ranked = vec![
            RankedSentence::new(0, 0.05, "first".to_string()),
            RankedSentence::new(1, 0.2, "second".to_string()),
        ];

        let selected = select_sentences(ranked, 2, 0.1);

        assert_eq!(selected.len(), 2);
        assert_eq!(selected[0].position, 0);
    }

    #[test]
    fn fallback_sentence_returns_first_non_empty() {
        let result = fallback_sentence("!!!\n\n有効な文です。  ");
        assert_eq!(result, Some((1, "有効な文です。".to_string())));
    }

    #[test]
    fn build_synthesis_prompt_lists_sentences() {
        let prompt = build_synthesis_prompt("本文", &vec!["文1".to_string(), "文2".to_string()]);

        assert!(prompt.contains("1. 文1"));
        assert!(prompt.contains("原文"));
    }
}
