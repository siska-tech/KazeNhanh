use std::error::Error as StdError;
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use git2::{Repository, Signature, Time};
use kaze_nhanh::{bench_support, GitNativeRagContent, GitReportOptions};
use tempfile::TempDir;

const BENCH_TEXT: &str = r#"
Rust provides fearless concurrency and strong memory-safety guarantees without garbage collection.
It empowers teams to build reliable systems while maintaining high performance and predictable latency.
When combined with thoughtful APIs and tooling, Rust adoption continues to grow across the industry.
"#;

fn bench_summarize(c: &mut Criterion) {
    let mut group = c.benchmark_group("summaries");
    group.throughput(Throughput::Bytes(BENCH_TEXT.len() as u64));
    group.bench_function(BenchmarkId::from_parameter("hybrid"), |b| {
        b.iter(|| {
            let result = bench_support::summarize(black_box(BENCH_TEXT))
                .expect("hybrid summary should succeed");
            black_box(result);
        });
    });
    group.finish();
}

fn bench_git_native_rag(c: &mut Criterion) {
    let mut group = c.benchmark_group("git_native_rag");

    group.bench_function("ingest_and_summarize", |b| {
        b.iter(|| {
            let fixture = TempRepo::markdown_fixture().expect("fixture should build");
            let options = fixture.git_options();

            let report =
                bench_support::run_git_native_rag(&options).expect("git native rag should run");
            assert!(matches!(
                report.content,
                GitNativeRagContent::Synthesized { .. }
            ));
            black_box(report);
        });
    });

    group.finish();
}

struct TempRepo {
    _dir: TempDir,
    path: String,
}

impl TempRepo {
    fn markdown_fixture() -> Result<Self, Box<dyn StdError>> {
        let dir = TempDir::new()?;
        let repo = Repository::init(dir.path())?;

        commit_markdown(
            &repo,
            "docs/guide.md",
            "# Guide\n\n- introduction\n",
            past_seconds(7200),
        )?;
        commit_markdown(
            &repo,
            "docs/guide.md",
            "# Guide\n\n- introduction\n- additional context\n",
            past_seconds(60),
        )?;

        Ok(Self {
            path: dir.path().to_string_lossy().into_owned(),
            _dir: dir,
        })
    }

    fn git_options(&self) -> GitReportOptions<'_> {
        GitReportOptions {
            repo_path: &self.path,
            days_since: 7,
            target_extensions: Some(vec![".md"]),
            custom_prompt: Some("Highlight developer impact."),
        }
    }
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
        .unwrap_or(Duration::from_secs(0))
        .as_secs() as i64;
    Time::new(now - seconds, 0)
}

criterion_group!(performance, bench_summarize, bench_git_native_rag);
criterion_main!(performance);
