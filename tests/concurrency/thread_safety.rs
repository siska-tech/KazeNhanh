use std::error::Error as StdError;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use git2::{Repository, Signature, Time};
use kaze_nhanh::{
    EngineConfig, GitNativeRagContent, GitNativeRagReport, GitReportOptions, KazeNhanhEngine,
};
use tempfile::TempDir;

const THREAD_COUNT: usize = 8;

fn test_engine() -> KazeNhanhEngine {
    let config = EngineConfig::new(b"model", b"dict", br#"{}"#);
    KazeNhanhEngine::new(config).expect("engine should initialize")
}

#[test]
fn summarize_with_details_is_thread_safe() {
    let engine = Arc::new(test_engine());
    let input = "Rust provides fearless concurrency and strong safety guarantees.";

    let handles: Vec<_> = (0..THREAD_COUNT)
        .map(|_| {
            let engine = Arc::clone(&engine);
            let text = format!(
                "{} Iteration {}.",
                input,
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            );

            thread::spawn(move || {
                for _ in 0..10 {
                    let (sentences, summary) = engine
                        .summarize_with_details(&text)
                        .expect("summary should succeed concurrently");
                    assert!(!sentences.is_empty());
                    assert!(summary.contains("mock-bytes"));
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("thread panicked");
    }
}

#[test]
fn git_native_rag_handles_parallel_invocations() -> Result<(), Box<dyn StdError>> {
    let engine = Arc::new(test_engine());

    let handles: Vec<_> = (0..THREAD_COUNT)
        .map(|idx| {
            let engine = Arc::clone(&engine);
            thread::spawn(move || -> Result<(), Box<dyn StdError>> {
                let repo_dir = TempDir::new()?;
                let repo = Repository::init(repo_dir.path())?;

                commit_markdown(
                    &repo,
                    "docs/notes.md",
                    "# Title\n\n- original\n",
                    past_seconds(7200 + idx as i64),
                )?;
                commit_markdown(
                    &repo,
                    "docs/notes.md",
                    format!("# Title\n\n- original\n- update {}\n", idx).as_str(),
                    past_seconds(60),
                )?;

                let repo_path = repo_dir.path().to_string_lossy().into_owned();
                let options = GitReportOptions {
                    repo_path: &repo_path,
                    days_since: 2,
                    target_extensions: Some(vec![".md"]),
                    custom_prompt: Some("Emphasize customer-facing changes."),
                };

                let report = engine.run_git_native_rag(&options)?;
                match report.content {
                    GitNativeRagContent::Synthesized { ref prompt, ref summary } => {
                        assert!(prompt.contains("docs/notes.md"));
                        assert!(summary.contains("mock-bytes"));
                    }
                    GitNativeRagContent::Fallback { .. } => {
                        panic!("expected synthesized report in concurrent execution")
                    }
                }

                Ok(())
            })
        })
        .collect();

    for handle in handles {
        handle
            .join()
            .expect("thread panicked during rag execution")?;
    }

    Ok(())
}

fn commit_markdown(
    repo: &Repository,
    relative_path: &str,
    contents: &str,
    timestamp: Time,
) -> Result<(), Box<dyn StdError>> {
    std::fs::create_dir_all(
        repo.workdir()
            .ok_or_else(|| "repository missing working directory")?
            .join("docs"),
    )?;

    std::fs::write(repo.workdir().unwrap().join(relative_path), contents)?;

    let mut index = repo.index()?;
    index.add_path(std::path::Path::new(relative_path))?;
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
        .unwrap_or(Duration::from_secs(0))
        .as_secs() as i64;
    Time::new(now - seconds, 0)
}

