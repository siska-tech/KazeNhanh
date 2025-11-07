use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::foundation::git_service::{
    collect_markdown_diffs, prepare_repository, FileDiff, LineAddition, RepositoryContext,
};
use crate::foundation::markdown::{MarkdownSection, MarkdownService};
use crate::inference::InferenceEngine;
use crate::{GitReportOptions, KazeNhanhError};
use git2::{ErrorClass, ErrorCode};

use super::correlation_engine::{correlate, ContextualChange};
use super::prompt_builder::{build_prompt, fallback_message};

pub(crate) struct GitNativeRAG {
    markdown_service: MarkdownService,
    inference_engine: Arc<Mutex<InferenceEngine>>,
}

impl GitNativeRAG {
    pub(crate) fn new(inference_engine: Arc<Mutex<InferenceEngine>>) -> Self {
        Self {
            markdown_service: MarkdownService::new(),
            inference_engine,
        }
    }

    pub(crate) fn ingest_diffs(
        &self,
        options: &GitReportOptions,
    ) -> Result<DiffIngestionResult, KazeNhanhError> {
        let context = match prepare_repository(options) {
            Ok(context) => context,
            Err(KazeNhanhError::GitOperationError { source, .. })
                if is_missing_head_reference(&source) =>
            {
                return Ok(DiffIngestionResult::empty(
                    options.repo_path.trim().to_string(),
                ));
            }
            Err(error) => return Err(error),
        };
        let extensions = options.effective_extensions();
        let diffs = collect_markdown_diffs(&context, &extensions)?;

        if diffs.is_empty() {
            return Ok(DiffIngestionResult::empty(context.repo_path.clone()));
        }

        let entries = self.enrich_diffs(&context, diffs)?;
        Ok(DiffIngestionResult::new(context.repo_path.clone(), entries))
    }

    pub(crate) fn execute(
        &self,
        options: &GitReportOptions,
    ) -> Result<GitNativeRagReport, KazeNhanhError> {
        let ingestion = self.ingest_diffs(options)?;

        if ingestion.entries.is_empty() {
            let message = fallback_message(ingestion.repo_path.as_str(), options.days_since);
            return Ok(GitNativeRagReport::fallback(ingestion.repo_path, message));
        }

        let contextual_changes = correlate(&ingestion.entries);

        if contextual_changes.is_empty() {
            let message = fallback_message(ingestion.repo_path.as_str(), options.days_since);
            return Ok(GitNativeRagReport::fallback(ingestion.repo_path, message));
        }

        let prompt = build_prompt(
            ingestion.repo_path.as_str(),
            options.days_since,
            &contextual_changes,
            options.custom_prompt,
        );

        let mut engine = self.lock_inference()?;
        let summary = match engine.synthesize(&prompt) {
            Ok(summary) => summary,
            Err(error @ KazeNhanhError::InvalidInput(_))
            | Err(error @ KazeNhanhError::ModelInferenceError { .. }) => {
                drop(engine);
                let message = format!(
                    "GitNativeRAG could not produce a summary for `{}`: {}",
                    ingestion.repo_path, error
                );
                return Ok(GitNativeRagReport::fallback(ingestion.repo_path, message));
            }
            Err(error) => return Err(error),
        };
        drop(engine);

        Ok(GitNativeRagReport::synthesized(
            ingestion.repo_path,
            contextual_changes,
            prompt,
            summary,
        ))
    }

    fn enrich_diffs(
        &self,
        context: &RepositoryContext,
        diffs: Vec<FileDiff>,
    ) -> Result<Vec<DiffIngestionEntry>, KazeNhanhError> {
        diffs
            .into_iter()
            .map(|diff| self.build_entry(context, diff))
            .collect()
    }

    fn build_entry(
        &self,
        context: &RepositoryContext,
        diff: FileDiff,
    ) -> Result<DiffIngestionEntry, KazeNhanhError> {
        let file_path = diff.path.clone();
        let absolute = Path::new(&context.repo_path).join(&file_path);
        let document = fs::read_to_string(&absolute)?;
        let sections = self.markdown_service.map_document_structure(&document)?;

        Ok(DiffIngestionEntry::new(
            file_path,
            diff.added_lines,
            sections,
            document,
        ))
    }

    fn lock_inference(&self) -> Result<MutexGuard<'_, InferenceEngine>, KazeNhanhError> {
        self.inference_engine.lock().map_err(|_| {
            KazeNhanhError::InvalidInput("Failed to lock inference engine".to_string())
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct DiffIngestionResult {
    pub repo_path: String,
    pub entries: Vec<DiffIngestionEntry>,
}

impl DiffIngestionResult {
    fn new(repo_path: String, entries: Vec<DiffIngestionEntry>) -> Self {
        Self { repo_path, entries }
    }

    fn empty(repo_path: String) -> Self {
        Self {
            repo_path,
            entries: Vec::new(),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct DiffIngestionEntry {
    pub file_path: String,
    pub added_lines: Vec<LineAddition>,
    pub sections: Vec<MarkdownSection>,
    pub document: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitNativeRagReport {
    pub repo_path: String,
    pub contextual_changes: Vec<ContextualChange>,
    pub content: GitNativeRagContent,
}

impl GitNativeRagReport {
    pub(crate) fn synthesized(
        repo_path: String,
        contextual_changes: Vec<ContextualChange>,
        prompt: String,
        summary: String,
    ) -> Self {
        Self {
            repo_path,
            contextual_changes,
            content: GitNativeRagContent::Synthesized { prompt, summary },
        }
    }

    pub(crate) fn fallback(repo_path: String, message: String) -> Self {
        Self {
            repo_path,
            contextual_changes: Vec::new(),
            content: GitNativeRagContent::Fallback { message },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitNativeRagContent {
    Synthesized { prompt: String, summary: String },
    Fallback { message: String },
}

fn is_missing_head_reference(error: &git2::Error) -> bool {
    matches!(error.class(), ErrorClass::Reference)
        && matches!(
            error.code(),
            ErrorCode::GenericError
                | ErrorCode::NotFound
                | ErrorCode::InvalidSpec
                | ErrorCode::UnbornBranch
        )
}

impl DiffIngestionEntry {
    fn new(
        file_path: String,
        added_lines: Vec<LineAddition>,
        sections: Vec<MarkdownSection>,
        document: String,
    ) -> Self {
        Self {
            file_path,
            added_lines,
            sections,
            document,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EngineConfig, GitReportOptions};
    use git2::{Repository, Signature};
    use std::error::Error as StdError;
    use std::fs;
    use std::path::Path;
    use std::sync::{Arc, Mutex};
    use tempfile::TempDir;

    #[test]
    fn ingest_diffs_returns_enriched_markdown_context() -> Result<(), Box<dyn StdError>> {
        let temp_dir = TempDir::new()?;
        let repo = Repository::init(temp_dir.path())?;

        commit_markdown(
            &repo,
            "docs/changelog.md",
            "# Changelog\n\n## Added\n- Implement ingestion\n",
        )?;

        let repo_path = temp_dir.path().to_string_lossy().into_owned();
        let options = GitReportOptions {
            repo_path: &repo_path,
            days_since: 7,
            target_extensions: Some(vec![".md"]),
            custom_prompt: None,
        };

        let config = EngineConfig::new(b"model", b"dict", br#"{}"#);
        let inference = Arc::new(Mutex::new(InferenceEngine::new(&config)?));
        let rag = GitNativeRAG::new(inference);

        let result = rag.ingest_diffs(&options)?;

        assert_eq!(result.repo_path, repo_path);
        assert_eq!(result.entries.len(), 1);

        let entry = &result.entries[0];
        assert_eq!(entry.file_path, "docs/changelog.md");
        assert_eq!(
            entry.document,
            "# Changelog\n\n## Added\n- Implement ingestion\n"
        );
        assert!(!entry.added_lines.is_empty());
        assert!(entry
            .sections
            .iter()
            .any(|section| section.title() == "Changelog"));

        Ok(())
    }

    #[test]
    fn ingest_diffs_returns_empty_when_no_commits() -> Result<(), Box<dyn StdError>> {
        let temp_dir = TempDir::new()?;
        let repo = Repository::init(temp_dir.path())?;

        assert!(repo.is_empty()?);

        let repo_path = temp_dir.path().to_string_lossy().into_owned();
        let options = GitReportOptions {
            repo_path: &repo_path,
            days_since: 7,
            target_extensions: Some(vec![".md"]),
            custom_prompt: None,
        };

        let config = EngineConfig::new(b"model", b"dict", br#"{}"#);
        let inference = Arc::new(Mutex::new(InferenceEngine::new(&config)?));
        let rag = GitNativeRAG::new(inference);

        let result = rag.ingest_diffs(&options)?;

        assert!(result.entries.is_empty());

        Ok(())
    }

    #[test]
    fn execute_returns_fallback_when_no_changes() -> Result<(), Box<dyn StdError>> {
        let temp_dir = TempDir::new()?;
        let repo = Repository::init(temp_dir.path())?;
        assert!(repo.is_empty()?);

        let repo_path = temp_dir.path().to_string_lossy().into_owned();
        let options = GitReportOptions {
            repo_path: &repo_path,
            days_since: 7,
            target_extensions: Some(vec![".md"]),
            custom_prompt: None,
        };

        let config = EngineConfig::new(b"model", b"dict", br#"{}"#);
        let inference = Arc::new(Mutex::new(InferenceEngine::new(&config)?));
        let rag = GitNativeRAG::new(inference);

        let report = rag.execute(&options)?;

        match report.content {
            GitNativeRagContent::Fallback { ref message } => {
                assert!(message.contains("No relevant"));
            }
            GitNativeRagContent::Synthesized { .. } => {
                panic!("expected fallback, found synthesized");
            }
        }

        assert!(report.contextual_changes.is_empty());
        assert_eq!(report.repo_path, repo_path);

        Ok(())
    }

    #[test]
    fn execute_runs_inference_for_changes() -> Result<(), Box<dyn StdError>> {
        let temp_dir = TempDir::new()?;
        let repo = Repository::init(temp_dir.path())?;

        commit_markdown(
            &repo,
            "docs/changelog.md",
            "# Title\n\n## Summary\n- initial\n",
        )?;

        commit_markdown(
            &repo,
            "docs/changelog.md",
            "# Title\n\n## Summary\n- initial\n- added point\n",
        )?;

        let repo_path = temp_dir.path().to_string_lossy().into_owned();
        let options = GitReportOptions {
            repo_path: &repo_path,
            days_since: 7,
            target_extensions: Some(vec![".md"]),
            custom_prompt: Some("Stress customer impact."),
        };

        let config = EngineConfig::new(b"model", b"dict", br#"{}"#);
        let inference = Arc::new(Mutex::new(InferenceEngine::new(&config)?));
        let rag = GitNativeRAG::new(inference);

        let report = rag.execute(&options)?;

        match report.content {
            GitNativeRagContent::Synthesized {
                ref prompt,
                ref summary,
            } => {
                assert!(prompt.contains("File: docs/changelog.md"));
                assert!(prompt.contains("Stress customer impact"));
                assert!(summary.contains("mock-bytes"));
            }
            GitNativeRagContent::Fallback { .. } => {
                panic!("expected synthesized result, found fallback");
            }
        }

        assert!(!report.contextual_changes.is_empty());
        assert_eq!(report.repo_path, repo_path);

        Ok(())
    }

    fn commit_markdown(
        repo: &Repository,
        relative_path: &str,
        contents: &str,
    ) -> Result<(), Box<dyn StdError>> {
        let workdir = repo
            .workdir()
            .ok_or_else(|| "repository missing working directory")?;
        let absolute_path = workdir.join(relative_path);
        if let Some(parent) = absolute_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&absolute_path, contents)?;

        let mut index = repo.index()?;
        index.add_path(Path::new(relative_path))?;
        index.write()?;
        let tree_id = index.write_tree()?;
        let tree = repo.find_tree(tree_id)?;

        let signature = Signature::now("tester", "tester@example.com")?;

        if repo.is_empty()? {
            repo.commit(Some("HEAD"), &signature, &signature, "initial", &tree, &[])?;
        } else {
            let head = repo.head()?.target().ok_or("missing HEAD target")?;
            let parent = repo.find_commit(head)?;
            repo.commit(
                Some("HEAD"),
                &signature,
                &signature,
                "update",
                &tree,
                &[&parent],
            )?;
        }

        Ok(())
    }
}
