use std::error::Error as StdError;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use git2::{Repository, Signature, Time};
use kaze_nhanh::{
    EngineConfig, GitNativeRagContent, GitNativeRagReport, GitReportOptions, KazeNhanhEngine,
};
use tempfile::TempDir;

fn test_engine() -> KazeNhanhEngine {
    let config = EngineConfig::new(b"model", b"dict", br#"{}"#);
    KazeNhanhEngine::new(config).expect("engine should initialize with mock assets")
}

#[test]
fn git_report_includes_markdown_additions() -> Result<(), Box<dyn StdError>> {
    let temp_dir = TempDir::new()?;
    let repo = Repository::init(temp_dir.path())?;

    commit_markdown(&repo, "docs/guide.md", "# Guide\n\n- intro\n", past_seconds(7200))?;
    commit_markdown(
        &repo,
        "docs/guide.md",
        "# Guide\n\n- intro\n- added insight\n",
        past_seconds(60),
    )?;

    let repo_path = temp_dir.path().to_string_lossy().into_owned();
    let options = GitReportOptions {
        repo_path: &repo_path,
        days_since: 7,
        target_extensions: Some(vec![".md"]),
        custom_prompt: None,
    };

    let engine = test_engine();
    let report = engine.generate_git_report(&options)?;

    assert!(report.contains("Git activity report"));
    assert!(report.contains("docs/guide.md"));
    assert!(report.contains("added insight"));

    Ok(())
}

#[test]
fn git_native_rag_synthesizes_summary_for_changes() -> Result<(), Box<dyn StdError>> {
    let temp_dir = TempDir::new()?;
    let repo = Repository::init(temp_dir.path())?;

    commit_markdown(&repo, "docs/changelog.md", "# Title\n\n- original\n", past_seconds(6400))?;
    commit_markdown(
        &repo,
        "docs/changelog.md",
        "# Title\n\n- original\n- customer impact\n",
        past_seconds(30),
    )?;

    let repo_path = temp_dir.path().to_string_lossy().into_owned();
    let options = GitReportOptions {
        repo_path: &repo_path,
        days_since: 2,
        target_extensions: Some(vec![".md"]),
        custom_prompt: Some("Highlight user-facing changes."),
    };

    let engine = test_engine();
    let report = engine.run_git_native_rag(&options)?;

    match report.content {
        GitNativeRagContent::Synthesized { ref prompt, ref summary } => {
            assert!(prompt.contains("docs/changelog.md"));
            assert!(prompt.contains("Highlight user-facing changes"));
            assert!(summary.contains("mock-bytes"));
        }
        GitNativeRagContent::Fallback { .. } => panic!("expected synthesized content"),
    }

    assert!(!report.contextual_changes.is_empty());

    Ok(())
}

#[test]
fn summarize_with_details_returns_sentences_and_summary() {
    let engine = test_engine();

    let input = "Rust ensures memory safety. It offers fearless concurrency.";
    let (sentences, summary) = engine
        .summarize_with_details(input)
        .expect("summaries should succeed");

    assert!(!sentences.is_empty());
    assert!(sentences.iter().any(|sentence| sentence.contains("Rust")));
    assert!(summary.contains("mock-bytes"));
}

fn commit_markdown(
    repo: &Repository,
    relative_path: &str,
    contents: &str,
    timestamp: Time,
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

    let signature = Signature::new("tester", "tester@example.com", &timestamp)?;

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

fn past_seconds(seconds: i64) -> Time {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    Time::new(now - seconds, 0)
}

