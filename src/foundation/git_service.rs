use crate::{GitReportOptions, KazeNhanhError};
use git2::{
    DiffDelta, DiffFlags, DiffHunk, DiffLine, DiffOptions, ErrorClass, ErrorCode, Oid, Repository,
    Sort, Time,
};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::{Duration, SystemTime};

const SECONDS_PER_DAY: u64 = 86_400;

/// Holds repository data that downstream diff logic can reuse.
pub(crate) struct RepositoryContext {
    pub repo_path: String,
    pub repository: Repository,
    pub since_time: Time,
    pub recent_commits: Vec<Oid>,
}

impl RepositoryContext {
    fn new(
        repo_path: String,
        repository: Repository,
        since_time: Time,
        recent_commits: Vec<Oid>,
    ) -> Self {
        Self {
            repo_path,
            repository,
            since_time,
            recent_commits,
        }
    }
}

/// Opens the repository described by `options` and prepares commit history constrained by `days_since`.
pub(crate) fn prepare_repository(
    options: &GitReportOptions,
) -> Result<RepositoryContext, KazeNhanhError> {
    options.validate()?;

    let repo_path = options.repo_path.trim().to_string();

    let repository = Repository::open(&repo_path).map_err(|err| {
        if err.code() == ErrorCode::NotFound
            && matches!(
                err.class(),
                ErrorClass::Repository | ErrorClass::Filesystem | ErrorClass::Os
            )
        {
            KazeNhanhError::RepositoryNotFound(repo_path.clone())
        } else {
            KazeNhanhError::git_operation_error(&repo_path, err)
        }
    })?;

    let since_time = calculate_since_time(options.days_since);
    let mut revwalk = repository
        .revwalk()
        .map_err(|err| KazeNhanhError::git_operation_error(&repo_path, err))?;
    revwalk
        .set_sorting(Sort::TIME)
        .map_err(|err| KazeNhanhError::git_operation_error(&repo_path, err))?;

    match revwalk.push_head() {
        Ok(()) => {}
        Err(err) if err.code() == ErrorCode::UnbornBranch => {
            drop(revwalk);
            return Ok(RepositoryContext::new(
                repo_path.clone(),
                repository,
                since_time,
                Vec::new(),
            ));
        }
        Err(err) => {
            return Err(KazeNhanhError::git_operation_error(&repo_path, err));
        }
    }

    let recent_commits = collect_recent_commits(&repo_path, &repository, revwalk, since_time)?;

    Ok(RepositoryContext::new(
        repo_path,
        repository,
        since_time,
        recent_commits,
    ))
}

fn collect_recent_commits(
    repo_path: &str,
    repository: &Repository,
    revwalk: git2::Revwalk,
    since_time: Time,
) -> Result<Vec<Oid>, KazeNhanhError> {
    let mut commits = Vec::new();

    for oid_result in revwalk {
        let oid = oid_result.map_err(|err| KazeNhanhError::git_operation_error(repo_path, err))?;
        let commit = repository
            .find_commit(oid)
            .map_err(|err| KazeNhanhError::git_operation_error(repo_path, err))?;

        if commit.time().seconds() < since_time.seconds() {
            break;
        }

        commits.push(oid);
    }

    Ok(commits)
}

fn calculate_since_time(days_since: u32) -> Time {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs();
    let since_seconds = u64::from(days_since).saturating_mul(SECONDS_PER_DAY);
    let threshold = now.saturating_sub(since_seconds);

    Time::new(threshold as i64, 0)
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct FileDiff {
    pub path: String,
    pub added_lines: Vec<LineAddition>,
}

impl FileDiff {
    fn new(path: String, added_lines: Vec<LineAddition>) -> Self {
        Self { path, added_lines }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub(crate) struct LineAddition {
    pub line_number: u32,
    pub content: String,
}

impl LineAddition {
    fn new(line_number: u32, content: String) -> Self {
        Self {
            line_number,
            content,
        }
    }
}

pub(crate) fn collect_markdown_diffs(
    context: &RepositoryContext,
    extensions: &[String],
) -> Result<Vec<FileDiff>, KazeNhanhError> {
    if context.recent_commits.is_empty() {
        return Ok(Vec::new());
    }

    let mut aggregated: BTreeMap<String, Vec<LineAddition>> = BTreeMap::new();

    for oid in &context.recent_commits {
        let commit = context
            .repository
            .find_commit(*oid)
            .map_err(|err| KazeNhanhError::git_operation_error(&context.repo_path, err))?;
        let tree = commit
            .tree()
            .map_err(|err| KazeNhanhError::git_operation_error(&context.repo_path, err))?;

        let parent_tree = if commit.parent_count() == 0 {
            None
        } else {
            let parent = commit
                .parent(0)
                .map_err(|err| KazeNhanhError::git_operation_error(&context.repo_path, err))?;
            Some(
                parent
                    .tree()
                    .map_err(|err| KazeNhanhError::git_operation_error(&context.repo_path, err))?,
            )
        };

        let mut diff_options = DiffOptions::new();
        diff_options.include_untracked(false);
        diff_options.context_lines(0);

        let diff = context
            .repository
            .diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut diff_options))
            .map_err(|err| KazeNhanhError::git_operation_error(&context.repo_path, err))?;

        let mut line_cb = |delta: DiffDelta, _hunk: Option<DiffHunk>, line: DiffLine| -> bool {
            if delta.flags().contains(DiffFlags::BINARY) {
                return true;
            }

            let path = match resolve_delta_path(&delta) {
                Some(path) => path,
                None => return true,
            };

            if !matches_extension(path, extensions) {
                return true;
            }

            if line.origin() != '+' {
                return true;
            }

            if let Some(line_number) = line.new_lineno() {
                let entry = aggregated
                    .entry(path.to_string_lossy().to_string())
                    .or_default();
                entry.push(LineAddition::new(
                    line_number,
                    normalize_line_content(line.content()),
                ));
            }

            true
        };

        let mut file_cb = |_: DiffDelta, _: f32| true;

        diff.foreach(&mut file_cb, None, None, Some(&mut line_cb))
            .map_err(|err| KazeNhanhError::git_operation_error(&context.repo_path, err))?;
    }

    let file_diffs: Vec<FileDiff> = aggregated
        .into_iter()
        .map(|(path, mut lines)| {
            lines.sort_by(|a, b| {
                a.line_number
                    .cmp(&b.line_number)
                    .then_with(|| a.content.cmp(&b.content))
            });
            FileDiff::new(path, lines)
        })
        .collect();

    Ok(file_diffs)
}

fn resolve_delta_path<'a>(delta: &'a DiffDelta<'_>) -> Option<&'a Path> {
    delta.new_file().path().or_else(|| delta.old_file().path())
}

fn matches_extension(path: &Path, extensions: &[String]) -> bool {
    let lower = path.to_string_lossy().to_ascii_lowercase();
    extensions
        .iter()
        .any(|ext| lower.ends_with(&ext.to_ascii_lowercase()))
}

fn normalize_line_content(content: &[u8]) -> String {
    let mut text = String::from_utf8_lossy(content).into_owned();
    while text.ends_with('\n') || text.ends_with('\r') {
        text.pop();
    }
    text
}

#[cfg(test)]
mod tests {
    use super::{
        calculate_since_time, collect_markdown_diffs, prepare_repository, SECONDS_PER_DAY,
    };
    use crate::GitReportOptions;
    use git2::{build::CheckoutBuilder, Repository, Signature, Time};
    use std::error::Error as StdError;
    use std::fs;
    use std::path::Path;
    use std::time::{Duration, SystemTime};
    use tempfile::TempDir;

    #[test]
    fn rejects_zero_days_since() {
        let options = GitReportOptions {
            repo_path: ".",
            days_since: 0,
            target_extensions: None,
            custom_prompt: None,
        };

        let error = prepare_repository(&options)
            .err()
            .expect("expected validation error");

        match error {
            crate::KazeNhanhError::InvalidInput(message) => {
                assert!(message.contains("days_since"));
            }
            _ => panic!("unexpected error variant"),
        }
    }

    #[test]
    fn returns_repository_not_found_for_missing_path() {
        let options = GitReportOptions {
            repo_path: "./path/does/not/exist",
            days_since: 1,
            target_extensions: None,
            custom_prompt: None,
        };

        let error = prepare_repository(&options)
            .err()
            .expect("expected repository lookup error");

        let display = error.to_string();

        match error {
            crate::KazeNhanhError::RepositoryNotFound(ref path) => {
                assert_eq!(path, options.repo_path);
                assert!(display.contains("./path/does/not/exist"));
            }
            other => panic!("unexpected error variant: {:?}", other),
        }
    }

    #[test]
    fn ignores_pure_deletion_commits() {
        let temp_dir = TempDir::new().expect("failed to create temp dir");
        let repo = Repository::init(temp_dir.path()).expect("failed to init repo");

        let now = current_unix_time();
        let old_commit_time = now - (4 * SECONDS_PER_DAY as i64);

        create_commit(&repo, "notes.md", "line1\nline2\n", old_commit_time);
        create_commit(&repo, "notes.md", "line1\n", now);

        let options = GitReportOptions {
            repo_path: temp_dir.path().to_str().unwrap(),
            days_since: 1,
            target_extensions: None,
            custom_prompt: None,
        };

        let context = prepare_repository(&options).expect("should prepare repository");
        let diffs = collect_markdown_diffs(&context, &options.effective_extensions())
            .expect("should collect diffs");

        assert!(diffs.is_empty());
    }

    #[test]
    fn rejects_invalid_target_extensions_without_dot() {
        let options = GitReportOptions {
            repo_path: ".",
            days_since: 1,
            target_extensions: Some(vec!["md"]),
            custom_prompt: None,
        };

        let error = prepare_repository(&options)
            .err()
            .expect("expected validation failure");

        match error {
            crate::KazeNhanhError::InvalidInput(message) => {
                assert!(message.contains("dot"));
            }
            _ => panic!("unexpected error variant"),
        }
    }

    #[test]
    fn rejects_duplicate_target_extensions_case_insensitive() {
        let options = GitReportOptions {
            repo_path: ".",
            days_since: 1,
            target_extensions: Some(vec![".md", ".MD"]),
            custom_prompt: None,
        };

        let error = prepare_repository(&options)
            .err()
            .expect("expected validation failure");

        match error {
            crate::KazeNhanhError::InvalidInput(message) => {
                assert!(message.contains("duplicates"));
            }
            _ => panic!("unexpected error variant"),
        }
    }

    #[test]
    fn rejects_blank_custom_prompt() {
        let options = GitReportOptions {
            repo_path: ".",
            days_since: 1,
            target_extensions: None,
            custom_prompt: Some("   "),
        };

        let error = prepare_repository(&options)
            .err()
            .expect("expected validation failure");

        match error {
            crate::KazeNhanhError::InvalidInput(message) => {
                assert!(message.contains("custom_prompt"));
            }
            _ => panic!("unexpected error variant"),
        }
    }

    #[test]
    fn filters_commits_by_days_since_threshold() {
        let temp_dir = TempDir::new().expect("failed to create temp dir");
        let repo = Repository::init(temp_dir.path()).expect("failed to init repo");

        let now = current_unix_time();
        let old_commit_time = now - (3 * SECONDS_PER_DAY as i64);
        let recent_commit_time = now - (SECONDS_PER_DAY as i64 / 2);

        create_commit(&repo, "README.md", "initial", old_commit_time);
        let recent_oid = create_commit(&repo, "README.md", "update", recent_commit_time);

        let options = GitReportOptions {
            repo_path: temp_dir.path().to_str().unwrap(),
            days_since: 2,
            target_extensions: None,
            custom_prompt: None,
        };

        let context = prepare_repository(&options).expect("should open repository");
        assert_eq!(context.recent_commits.len(), 1);
        assert_eq!(context.recent_commits[0], recent_oid);

        // Ensure the since_time roughly matches the expected window.
        let expected_threshold = now - (options.days_since as i64 * SECONDS_PER_DAY as i64);
        assert!(context.since_time.seconds() <= now);
        assert!(context.since_time.seconds() >= expected_threshold);
    }

    #[test]
    fn calculate_since_time_saturates_when_window_exceeds_now() {
        let time = calculate_since_time(u32::MAX);
        assert!(time.seconds() >= 0);
    }

    #[test]
    fn collects_markdown_diffs_from_merge_commits() {
        let temp_dir = TempDir::new().expect("failed to create temp dir");
        let repo = Repository::init(temp_dir.path()).expect("failed to init repo");

        let mut checkout = CheckoutBuilder::new();
        checkout.force();

        let now = current_unix_time();
        let base_time = now - (5 * SECONDS_PER_DAY as i64);
        let feature_time = now - (4 * SECONDS_PER_DAY as i64);
        let master_time = now - (SECONDS_PER_DAY as i64);
        let merge_time = now;

        let base_oid = create_commit(&repo, "notes.md", "base\n", base_time);

        let base_commit = repo
            .find_commit(base_oid)
            .expect("failed to find base commit");
        repo.branch("feature", &base_commit, false)
            .expect("failed to create feature branch");
        repo.set_head("refs/heads/feature")
            .expect("failed to set head to feature");
        repo.checkout_head(Some(&mut checkout))
            .expect("failed to checkout feature branch");

        let feature_oid = create_commit(&repo, "notes.md", "base\nfeature line\n", feature_time);

        repo.set_head("refs/heads/master")
            .expect("failed to set head to master");
        repo.checkout_head(Some(&mut checkout))
            .expect("failed to checkout master branch");

        let master_oid = create_commit(&repo, "notes.md", "base\nmaster line\n", master_time);

        let feature_commit = repo
            .find_commit(feature_oid)
            .expect("failed to find feature commit");
        let master_commit = repo
            .find_commit(master_oid)
            .expect("failed to find master commit");

        let merge_contents = "base\nmaster line\nfeature line\n";
        let workdir = repo.workdir().expect("expected non-bare repo");
        fs::write(workdir.join("notes.md"), merge_contents)
            .expect("failed to write merge contents");

        let mut index = repo.index().expect("failed to open index");
        index
            .add_path(Path::new("notes.md"))
            .expect("failed to add notes.md");
        index.write().expect("failed to write index");
        let tree_id = index.write_tree().expect("failed to write tree");
        let tree = repo.find_tree(tree_id).expect("failed to find tree");

        let signature = Signature::new("Tester", "tester@example.com", &Time::new(merge_time, 0))
            .expect("failed to create signature");

        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "merge commit",
            &tree,
            &[&master_commit, &feature_commit],
        )
        .expect("failed to create merge commit");

        repo.checkout_head(Some(&mut checkout))
            .expect("failed to refresh working tree");

        let options = GitReportOptions {
            repo_path: temp_dir.path().to_str().unwrap(),
            days_since: 2,
            target_extensions: None,
            custom_prompt: None,
        };

        let context = prepare_repository(&options).expect("should prepare repository");
        let diffs = collect_markdown_diffs(&context, &options.effective_extensions())
            .expect("should collect diffs");

        let additions: Vec<_> = diffs
            .iter()
            .flat_map(|diff| &diff.added_lines)
            .map(|line| line.content.as_str())
            .collect();

        assert!(additions.iter().any(|content| *content == "feature line"));
    }

    #[test]
    fn collects_markdown_diffs_across_commits() {
        let temp_dir = TempDir::new().expect("failed to create temp dir");
        let repo = Repository::init(temp_dir.path()).expect("failed to init repo");

        let now = current_unix_time();
        create_commit(&repo, "notes.md", "# Title\n", now - SECONDS_PER_DAY as i64);
        create_commit(&repo, "notes.md", "# Title\nSecond line\n", now);

        let options = GitReportOptions {
            repo_path: temp_dir.path().to_str().unwrap(),
            days_since: 7,
            target_extensions: None,
            custom_prompt: None,
        };

        let context = prepare_repository(&options).expect("should prepare repository");
        let diffs = collect_markdown_diffs(&context, &options.effective_extensions())
            .expect("should collect diffs");

        assert_eq!(diffs.len(), 1);
        let diff = &diffs[0];
        assert!(diff.path.ends_with("notes.md"));
        assert_eq!(diff.added_lines.len(), 2);
        assert_eq!(diff.added_lines[0].line_number, 1);
        assert_eq!(diff.added_lines[0].content, "# Title");
        assert_eq!(diff.added_lines[1].line_number, 2);
        assert_eq!(diff.added_lines[1].content, "Second line");
    }

    #[test]
    fn surfaces_git_operation_error_when_commit_is_missing() {
        let temp_dir = TempDir::new().expect("failed to create temp dir");
        let repo = Repository::init(temp_dir.path()).expect("failed to init repo");

        let now = current_unix_time();
        let oid = create_commit(&repo, "notes.md", "content", now);

        let hex = oid.to_string();
        let (dir, file) = hex.split_at(2);
        let object_path = repo.path().join("objects").join(dir).join(file);
        fs::remove_file(&object_path).expect("failed to remove commit object");

        let repo_path = temp_dir.path().to_str().expect("path to str");
        let options = GitReportOptions {
            repo_path,
            days_since: 1,
            target_extensions: None,
            custom_prompt: None,
        };

        let error = prepare_repository(&options)
            .err()
            .expect("expected repository corruption error");

        let message = error.to_string();

        match &error {
            crate::KazeNhanhError::GitOperationError { path, .. } => {
                assert_eq!(path, repo_path.trim());
                assert!(message.contains(repo_path));
            }
            other => panic!("unexpected error variant: {:?}", other),
        }

        let source = error
            .source()
            .expect("expected git2 source error for GitOperationError");
        assert!(source.to_string().contains("object"));
    }

    #[test]
    fn skips_non_markdown_and_binary_diffs() {
        let temp_dir = TempDir::new().expect("failed to create temp dir");
        let repo = Repository::init(temp_dir.path()).expect("failed to init repo");

        let now = current_unix_time();
        create_commit(
            &repo,
            "notes.md",
            "Introduction\n",
            now - SECONDS_PER_DAY as i64,
        );
        create_commit_with_bytes(&repo, "binary.md", &[0, 159, 146, 150, 0], now - 100);
        create_commit(&repo, "notes.txt", "ignored\n", now);

        let options = GitReportOptions {
            repo_path: temp_dir.path().to_str().unwrap(),
            days_since: 7,
            target_extensions: None,
            custom_prompt: None,
        };

        let context = prepare_repository(&options).expect("should prepare repository");
        let diffs = collect_markdown_diffs(&context, &options.effective_extensions())
            .expect("should collect diffs");

        assert_eq!(diffs.len(), 1);
        let diff = &diffs[0];
        assert!(diff.path.ends_with("notes.md"));
        assert_eq!(diff.added_lines.len(), 1);
        assert_eq!(diff.added_lines[0].line_number, 1);
        assert_eq!(diff.added_lines[0].content, "Introduction");
    }

    #[test]
    fn collects_and_sorts_added_markdown_lines_across_commits() {
        let temp_dir = TempDir::new().expect("failed to create temp dir");
        let repo = Repository::init(temp_dir.path()).expect("failed to init repo");

        let now = current_unix_time();
        create_commit(
            &repo,
            "docs/notes.md",
            "# Title\n\n- base\n",
            now - (5 * SECONDS_PER_DAY as i64),
        );
        create_commit(
            &repo,
            "docs/notes.md",
            "# Title\n\n- base\n- feature a\n",
            now - (SECONDS_PER_DAY as i64) / 2,
        );
        create_commit(
            &repo,
            "docs/notes.md",
            "# Title\n\n- base\n- feature a\n- feature b\n",
            now - 60,
        );

        let options = GitReportOptions {
            repo_path: temp_dir.path().to_str().unwrap(),
            days_since: 1,
            target_extensions: Some(vec![".md"]),
            custom_prompt: None,
        };

        let context = prepare_repository(&options).expect("should prepare repository");
        let diffs = collect_markdown_diffs(&context, &options.effective_extensions())
            .expect("should collect diffs");

        assert_eq!(diffs.len(), 1);
        let diff = &diffs[0];
        assert!(diff.path.ends_with("docs/notes.md"));
        assert_eq!(diff.added_lines.len(), 2);
        assert_eq!(diff.added_lines[0].line_number, 4);
        assert_eq!(diff.added_lines[0].content, "- feature a");
        assert_eq!(diff.added_lines[1].line_number, 5);
        assert_eq!(diff.added_lines[1].content, "- feature b");
    }

    fn create_commit(repo: &Repository, file: &str, contents: &str, timestamp: i64) -> git2::Oid {
        create_commit_with_bytes(repo, file, contents.as_bytes(), timestamp)
    }

    fn create_commit_with_bytes(
        repo: &Repository,
        file: &str,
        contents: &[u8],
        timestamp: i64,
    ) -> git2::Oid {
        let workdir = repo.workdir().expect("expected non-bare repo");
        let path = workdir.join(file);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("failed to create parent directories");
        }
        fs::write(&path, contents).expect("failed to write file");

        let mut index = repo.index().expect("failed to get index");
        index
            .add_path(Path::new(file))
            .expect("failed to add file to index");
        index.write().expect("failed to write index");
        let tree_id = index.write_tree().expect("failed to write tree");
        let tree = repo.find_tree(tree_id).expect("failed to find tree");

        let signature = Signature::new("Tester", "tester@example.com", &Time::new(timestamp, 0))
            .expect("failed to create signature");

        let parents: Vec<git2::Commit> = repo
            .head()
            .ok()
            .and_then(|head| head.target())
            .map(|oid| repo.find_commit(oid).expect("failed to find parent commit"))
            .into_iter()
            .collect();
        let parent_refs: Vec<&git2::Commit> = parents.iter().collect();

        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "test commit",
            &tree,
            &parent_refs,
        )
        .expect("failed to create commit")
    }

    fn current_unix_time() -> i64 {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs() as i64
    }
}
