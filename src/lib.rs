//! Core library entry point for the KazeNhanh engine crate.
//! Currently exposes configuration primitives used to bootstrap services.

pub mod foundation;
mod inference;
mod pipeline;

use std::sync::{Arc, Mutex, MutexGuard};
use thiserror::Error;

use foundation::git_service::{collect_markdown_diffs, prepare_repository, FileDiff};
pub use foundation::nlp::TokenizedMorpheme;
use foundation::nlp::{split_sentences, NlpService};
use inference::InferenceEngine;
pub use pipeline::git_native_rag::{GitNativeRagContent, GitNativeRagReport};
use pipeline::{GitNativeRAG, HybridSummarizer, HybridSummarizerConfig};

/// Unified error type exposed by the KazeNhanh crate.
#[derive(Debug, Error)]
pub enum KazeNhanhError {
    /// Wrapper around git2 errors that occur after repository discovery.
    #[error("Gitリポジトリの操作に失敗しました (パス: {path})")]
    GitOperationError {
        path: String,
        #[source]
        source: git2::Error,
    },

    /// Returned when the repository path does not exist or is not a Git repository.
    #[error("リポジトリが見つかりません: {0}")]
    RepositoryNotFound(String),

    /// Raised when the GGUF model or related assets cannot be loaded.
    #[error("AIモデルのロードに失敗しました")]
    ModelLoadError {
        #[source]
        source: candle_core::Error,
    },

    /// Raised when inference execution fails while generating summaries.
    #[error("AIモデルでの推論（合成）中にエラーが発生しました")]
    ModelInferenceError {
        #[source]
        source: candle_core::Error,
    },

    /// Raised when Sudachi resources fail to load during engine initialization.
    #[error("日本語辞書または設定ファイルのロードに失敗しました")]
    DictionaryLoadError {
        #[source]
        source: sudachi::error::SudachiError,
    },

    /// Raised when Sudachi fails to tokenize the provided input.
    #[error("日本語の形態素解析に失敗しました")]
    TokenizationError {
        #[source]
        source: sudachi::error::SudachiError,
    },

    /// Raised when Markdown parsing or offset mapping fails.
    #[error("Markdownの解析または行番号のマッピングに失敗しました: {0}")]
    MarkdownParseError(String),

    /// Raised when Git diff hunks or lines cannot be parsed correctly.
    #[error("Git差分(diff)の解析に失敗しました: {0}")]
    DiffParseError(String),

    /// Raised when the summarization graph algorithms fail.
    #[error("要約エンジンでエラーが発生しました: {0}")]
    SummarizeEngineError(String),

    /// Returned when user-provided input fails validation.
    #[error("無効な入力が指定されました: {0}")]
    InvalidInput(String),

    /// Raised when an unexpected I/O error occurs.
    #[error("予期せぬI/Oエラーが発生しました")]
    IoError(#[from] std::io::Error),
}

impl KazeNhanhError {
    pub(crate) fn git_operation_error(path: &str, source: git2::Error) -> Self {
        Self::GitOperationError {
            path: path.to_string(),
            source,
        }
    }
}

/// High-level facade aggregating inference and NLP services.
pub struct KazeNhanhEngine {
    inference_engine: Arc<Mutex<InferenceEngine>>,
    nlp_service: Arc<NlpService>,
    hybrid_summarizer: HybridSummarizer,
    git_native_rag: GitNativeRAG,
}

impl KazeNhanhEngine {
    /// Constructs a new engine by loading inference and NLP services from static resources.
    pub fn new(config: EngineConfig) -> Result<Self, KazeNhanhError> {
        let nlp_service = NlpService::new(&config)
            .map_err(|source| KazeNhanhError::DictionaryLoadError { source })?;

        let inference_engine = InferenceEngine::new(&config)?;

        let nlp_service = Arc::new(nlp_service);
        let inference_engine = Arc::new(Mutex::new(inference_engine));
        let tokenizer: Arc<dyn pipeline::SentenceTokenizer + Send + Sync> = nlp_service.clone();
        let hybrid_config = HybridSummarizerConfig::default();
        let hybrid_summarizer =
            HybridSummarizer::new(tokenizer, inference_engine.clone(), hybrid_config);
        let git_native_rag = GitNativeRAG::new(inference_engine.clone());

        Ok(Self {
            inference_engine,
            nlp_service,
            hybrid_summarizer,
            git_native_rag,
        })
    }

    /// Generates a Git activity report based on Markdown additions within the requested window.
    pub fn generate_git_report(
        &self,
        options: &GitReportOptions,
    ) -> Result<String, KazeNhanhError> {
        let context = prepare_repository(options)?;
        let extensions = options.effective_extensions();
        let diffs = collect_markdown_diffs(&context, &extensions)?;

        if diffs.is_empty() {
            return Ok(format!(
                "No Markdown changes detected for `{}` in the last {} day(s).",
                options.repo_path, options.days_since
            ));
        }

        let sections = diffs.iter().map(render_diff_summary).collect::<Vec<_>>();

        Ok(format!(
            "Git activity report for `{}`\n\n{}",
            options.repo_path,
            sections.join("\n\n")
        ))
    }

    /// Executes the GitNativeRAG workflow and returns synthesized output or fallback messaging.
    pub fn run_git_native_rag(
        &self,
        options: &GitReportOptions,
    ) -> Result<GitNativeRagReport, KazeNhanhError> {
        match self.git_native_rag.execute(options) {
            Ok(report) => Ok(report),
            Err(KazeNhanhError::InvalidInput(message)) => {
                let repo_path = options.repo_path.trim().to_string();
                let fallback_message = format!(
                    "GitNativeRAG request was invalid for `{}`: {}",
                    repo_path, message
                );
                Ok(GitNativeRagReport::fallback(repo_path, fallback_message))
            }
            Err(error) => Err(error),
        }
    }

    /// Produces a hybrid extractive-abstractive summary using the configured pipeline.
    pub fn summarize_document(&self, text: &str) -> Result<String, KazeNhanhError> {
        let (_, summary) = self.summarize_with_details(text)?;
        Ok(summary)
    }

    /// Returns both the extracted sentences and synthesized paragraph.
    pub fn summarize_with_details(
        &self,
        text: &str,
    ) -> Result<(Vec<String>, String), KazeNhanhError> {
        if text.trim().is_empty() {
            return Err(KazeNhanhError::InvalidInput(
                "summarize_document received empty text".to_string(),
            ));
        }

        let summary = self.hybrid_summarizer.execute(text)?;
        let extracted_sentences = summary.extracted_sentences;
        let synthesized_summary = summary.synthesized_summary.ok_or_else(|| {
            KazeNhanhError::SummarizeEngineError(
                "hybrid summarizer produced no synthesis".to_string(),
            )
        })?;

        Ok((extracted_sentences, synthesized_summary))
    }

    /// Synthesizes a summary paragraph from the provided sentences.
    pub fn synthesize_summary(&self, sentences: Vec<String>) -> Result<String, KazeNhanhError> {
        if sentences.is_empty() {
            return Err(KazeNhanhError::InvalidInput(
                "synthesize_summary requires at least one sentence".to_string(),
            ));
        }

        let cleaned = sentences
            .into_iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>();

        if cleaned.is_empty() {
            return Err(KazeNhanhError::InvalidInput(
                "synthesize_summary received only blank sentences".to_string(),
            ));
        }

        let mut engine = self.lock_inference()?;
        engine.synthesize(&cleaned.join("\n"))
    }

    /// Extracts candidate sentences using a simple segmentation heuristic.
    pub fn extract_important_sentences(
        &self,
        text: &str,
        count: usize,
    ) -> Result<Vec<String>, KazeNhanhError> {
        if count == 0 {
            return Ok(Vec::new());
        }

        if text.trim().is_empty() {
            return Err(KazeNhanhError::InvalidInput(
                "extract_important_sentences received empty text".to_string(),
            ));
        }

        let sentences = split_sentences(text);
        if sentences.is_empty() {
            return Ok(Vec::new());
        }

        Ok(sentences.into_iter().take(count).collect())
    }

    /// Tokenizes the supplied sentence using the underlying Sudachi service.
    pub fn tokenize_sentence(
        &self,
        sentence: &str,
    ) -> Result<Vec<TokenizedMorpheme>, KazeNhanhError> {
        self.nlp_service
            .tokenize(sentence)
            .map_err(|source| KazeNhanhError::TokenizationError { source })
    }

    fn lock_inference(&self) -> Result<MutexGuard<'_, InferenceEngine>, KazeNhanhError> {
        self.inference_engine.lock().map_err(|_| {
            KazeNhanhError::InvalidInput("Failed to lock inference engine".to_string())
        })
    }
}

#[cfg(feature = "mock_inference")]
pub mod bench_support {
    use std::sync::{Arc, Mutex};

    use sudachi::error::SudachiError;

    use crate::foundation::nlp_service::TokenizedMorpheme;
    use crate::inference::InferenceEngine;
    use crate::pipeline::git_native_rag::GitNativeRAG;
    use crate::pipeline::hybrid_summarizer::{HybridSummarizer, HybridSummarizerConfig};
    use crate::pipeline::sentence::SentenceTokenizer;
    use crate::{EngineConfig, GitNativeRagReport, GitReportOptions, KazeNhanhError};

    #[derive(Default)]
    struct BenchTokenizer;

    impl SentenceTokenizer for BenchTokenizer {
        fn tokenize(&self, text: &str) -> Result<Vec<TokenizedMorpheme>, SudachiError> {
            let mut results = Vec::new();
            let mut char_index = 0;

            for token in text.split_whitespace() {
                let start = char_index;
                let end = start + token.len();
                results.push(mock_morpheme(token, start, end));
                char_index = end + 1;
            }

            Ok(results)
        }
    }

    fn mock_morpheme(token: &str, begin: usize, end: usize) -> TokenizedMorpheme {
        TokenizedMorpheme::new(
            token.to_string(),
            token.to_string(),
            token.to_string(),
            token.to_string(),
            vec!["名詞".to_string()],
            0,
            false,
            begin,
            end,
            begin,
            end,
            Vec::new(),
            1,
        )
    }

    fn inference_engine() -> Result<InferenceEngine, KazeNhanhError> {
        let config = EngineConfig::new(b"model", b"dict", br#"{}"#);
        InferenceEngine::new(&config)
    }

    fn summarizer() -> Result<HybridSummarizer, KazeNhanhError> {
        let tokenizer: Arc<dyn SentenceTokenizer + Send + Sync> =
            Arc::new(BenchTokenizer::default());
        let engine = Arc::new(Mutex::new(inference_engine()?));
        let config = HybridSummarizerConfig::default();
        Ok(HybridSummarizer::new(tokenizer, engine, config))
    }

    pub fn summarize(text: &str) -> Result<(Vec<String>, String), KazeNhanhError> {
        let summarizer = summarizer()?;
        let summary = summarizer.execute(text)?;
        let extracted = summary.extracted_sentences;
        let synthesized = summary
            .synthesized_summary
            .unwrap_or_else(|| String::from(""));
        Ok((extracted, synthesized))
    }

    pub fn run_git_native_rag(
        options: &GitReportOptions,
    ) -> Result<GitNativeRagReport, KazeNhanhError> {
        let engine = Arc::new(Mutex::new(inference_engine()?));
        let rag = GitNativeRAG::new(engine);
        rag.execute(options)
    }
}

/// Options passed into the Git report pipeline.
#[derive(Debug, Clone)]
pub struct GitReportOptions<'a> {
    /// Path to the target Git repository.
    pub repo_path: &'a str,
    /// Number of days to look back when collecting commits.
    pub days_since: u32,
    /// Optional list of target file extensions to filter diffs.
    pub target_extensions: Option<Vec<&'a str>>,
    /// Optional custom prompt for downstream AI synthesis.
    pub custom_prompt: Option<&'a str>,
}

impl<'a> GitReportOptions<'a> {
    /// Validates the supplied options.
    pub(crate) fn validate(&self) -> Result<(), KazeNhanhError> {
        let trimmed_repo_path = self.repo_path.trim();

        if trimmed_repo_path.is_empty() {
            return Err(KazeNhanhError::InvalidInput(
                "リポジトリパスが空です".to_string(),
            ));
        }

        if self.days_since == 0 {
            return Err(KazeNhanhError::InvalidInput(
                "days_since must be greater than 0".to_string(),
            ));
        }

        if let Some(extensions) = &self.target_extensions {
            if extensions.is_empty() {
                return Err(KazeNhanhError::InvalidInput(
                    "target_extensions must not be empty".to_string(),
                ));
            }

            use std::collections::HashSet;

            let mut seen = HashSet::new();
            for ext in extensions {
                let trimmed = ext.trim();

                if trimmed.is_empty() {
                    return Err(KazeNhanhError::InvalidInput(
                        "target_extensions contains a blank value".to_string(),
                    ));
                }

                if trimmed.chars().any(char::is_whitespace) {
                    return Err(KazeNhanhError::InvalidInput(format!(
                        "target_extensions contain whitespace characters: `{}`",
                        trimmed
                    )));
                }

                if !trimmed.starts_with('.') {
                    return Err(KazeNhanhError::InvalidInput(format!(
                        "target_extensions must start with a dot: `{}`",
                        trimmed
                    )));
                }

                if trimmed.contains(['/', '\\']) {
                    return Err(KazeNhanhError::InvalidInput(format!(
                        "target_extensions must not contain path separators: `{}`",
                        trimmed
                    )));
                }

                let normalized = trimmed.to_ascii_lowercase();
                if !seen.insert(normalized) {
                    return Err(KazeNhanhError::InvalidInput(format!(
                        "target_extensions contains duplicates: `{}`",
                        trimmed
                    )));
                }
            }
        }

        if let Some(prompt) = self.custom_prompt {
            if prompt.trim().is_empty() {
                return Err(KazeNhanhError::InvalidInput(
                    "custom_prompt must not be blank".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Returns the list of extensions to evaluate, falling back to Markdown defaults.
    pub(crate) fn effective_extensions(&self) -> Vec<String> {
        match &self.target_extensions {
            Some(values) if !values.is_empty() => values
                .iter()
                .map(|ext| ext.trim().to_ascii_lowercase())
                .collect(),
            _ => vec![".md".to_string(), ".markdown".to_string()],
        }
    }
}

const MAX_DIFF_LINES: usize = 10;

fn render_diff_summary(diff: &FileDiff) -> String {
    let mut snippet = Vec::new();

    for line in diff.added_lines.iter().take(MAX_DIFF_LINES) {
        snippet.push(format!("  +{} {}", line.line_number, line.content));
    }

    if diff.added_lines.len() > MAX_DIFF_LINES {
        snippet.push(format!(
            "  ... (+{} more lines)",
            diff.added_lines.len() - MAX_DIFF_LINES
        ));
    }

    format!("### {}\n{}", diff.path, snippet.join("\n"))
}

/// Configuration bundle of static resources required to bootstrap `KazeNhanhEngine`.
///
/// Each field stores a byte slice that must live for the `'static` lifetime so it can
/// be embedded with `include_bytes!` or other compile-time resource loaders. This
/// satisfies the zero-setup requirement outlined in the API specification.
///
/// # Examples
/// ```
/// use kaze_nhanh::EngineConfig;
///
/// const MODEL_BYTES: &[u8] = b"gguf";
/// const DICT_BYTES: &[u8] = b"sudachi";
/// const SETTINGS_BYTES: &[u8] = br#"{"dict":"system"}"#;
///
/// let config = EngineConfig::new(MODEL_BYTES, DICT_BYTES, SETTINGS_BYTES);
/// assert_eq!(config.model_bytes.len(), MODEL_BYTES.len());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EngineConfig {
    /// Serialized GGUF model bytes consumed by the inference backend.
    pub model_bytes: &'static [u8],
    /// Sudachi dictionary (system.dic) bytes used by the NLP tokenizer service.
    pub dictionary_bytes: &'static [u8],
    /// Sudachi settings JSON bytes used to configure the tokenizer.
    pub settings_bytes: &'static [u8],
}

impl EngineConfig {
    /// Creates a new `EngineConfig` from statically-embedded resources.
    #[must_use]
    pub const fn new(
        model_bytes: &'static [u8],
        dictionary_bytes: &'static [u8],
        settings_bytes: &'static [u8],
    ) -> Self {
        Self {
            model_bytes,
            dictionary_bytes,
            settings_bytes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{EngineConfig, GitReportOptions, KazeNhanhEngine, KazeNhanhError};
    use candle_core::Error as CandleError;
    use std::io::{self, ErrorKind};
    use sudachi::error::SudachiError;

    const MODEL_BYTES: &[u8] = b"gguf-model";
    const DICT_BYTES: &[u8] = b"sudachi-dictionary";
    const SETTINGS_BYTES: &[u8] = br#"{"dict":"system"}"#;

    #[test]
    fn constructs_engine_config_from_static_resources() {
        let config = EngineConfig::new(MODEL_BYTES, DICT_BYTES, SETTINGS_BYTES);

        assert_eq!(config.model_bytes, MODEL_BYTES);
        assert_eq!(config.dictionary_bytes, DICT_BYTES);
        assert_eq!(config.settings_bytes, SETTINGS_BYTES);
    }

    #[test]
    fn effective_extensions_normalize_whitespace_and_case() {
        let options = GitReportOptions {
            repo_path: ".",
            days_since: 1,
            target_extensions: Some(vec![" .MD ", ".MarkDown "]),
            custom_prompt: None,
        };

        let extensions = options.effective_extensions();

        assert_eq!(extensions, vec![".md".to_string(), ".markdown".to_string()]);
    }

    #[test]
    fn engine_initialization_maps_dictionary_errors() {
        let config = EngineConfig::new(b"model", b"invalid", br#"{}"#);

        let error = KazeNhanhEngine::new(config)
            .err()
            .expect("expected dictionary load error");

        match error {
            KazeNhanhError::DictionaryLoadError { .. } => {}
            other => panic!("unexpected error variant: {:?}", other),
        }
    }

    #[test]
    fn validate_rejects_duplicate_extensions() {
        let options = GitReportOptions {
            repo_path: ".",
            days_since: 2,
            target_extensions: Some(vec![".MD", ".md"]),
            custom_prompt: Some("Focus on customer impact."),
        };

        let error = options
            .validate()
            .expect_err("expected duplicate extension validation error");

        match error {
            KazeNhanhError::InvalidInput(message) => {
                assert!(message.contains("duplicates"));
            }
            other => panic!("unexpected error variant: {:?}", other),
        }
    }

    #[test]
    fn validate_accepts_trimmed_inputs() {
        let options = GitReportOptions {
            repo_path: " ./repo ",
            days_since: 7,
            target_extensions: Some(vec![" .RST ", ".md"]),
            custom_prompt: Some(" Highlight risk areas. "),
        };

        options.validate().expect("validation should succeed");
    }

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn engine_is_send_and_sync() {
        assert_send_sync::<KazeNhanhEngine>();
    }

    #[test]
    fn converts_io_error_via_from() {
        fn trigger_io_error() -> io::Result<()> {
            Err(io::Error::new(ErrorKind::Other, "disk failure"))
        }

        fn wrap() -> Result<(), KazeNhanhError> {
            trigger_io_error()?;
            Ok(())
        }

        match wrap().expect_err("expected IoError variant") {
            KazeNhanhError::IoError(err) => {
                assert_eq!(err.kind(), ErrorKind::Other);
                assert!(err.to_string().contains("disk failure"));
            }
            other => panic!("unexpected error variant: {:?}", other),
        }
    }

    #[test]
    fn maps_candle_error_to_model_load_error() {
        fn load_model() -> Result<(), CandleError> {
            Err(CandleError::Msg("model boom".into()))
        }

        fn wrap() -> Result<(), KazeNhanhError> {
            load_model().map_err(|source| KazeNhanhError::ModelLoadError { source })?;
            Ok(())
        }

        match wrap().expect_err("expected ModelLoadError") {
            KazeNhanhError::ModelLoadError { source } => {
                assert!(source.to_string().contains("model boom"));
            }
            other => panic!("unexpected error variant: {:?}", other),
        }
    }

    #[test]
    fn maps_sudachi_error_to_tokenization_error() {
        fn tokenize() -> Result<(), SudachiError> {
            Err(SudachiError::InvalidDictionaryGrammar)
        }

        fn wrap() -> Result<(), KazeNhanhError> {
            tokenize().map_err(|source| KazeNhanhError::TokenizationError { source })?;
            Ok(())
        }

        match wrap().expect_err("expected TokenizationError") {
            KazeNhanhError::TokenizationError { source } => {
                assert!(source.to_string().contains("Invalid"));
            }
            other => panic!("unexpected error variant: {:?}", other),
        }
    }
}
