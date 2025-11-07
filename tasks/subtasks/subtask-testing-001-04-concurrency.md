---
status: completed
priority: medium
assignee: Backend
parent: task-testing-001-quality-assurance
start_date: 2025-11-07
end_date: 2025-11-07
tags: ["testing", "concurrency"]
depends_on: ["subtask-testing-001-03-integration"]
---

# サブタスク概要
並行性テストを構築し、`-race`実行時にデータ競合やデッドロックが発生しないことを検証する。

## 完了条件
- 主要APIに対する高並行アクセスシナリオがテスト化されている。
- `go test -race`でテストスイートがパスする。
- レース条件の検出やロック争いに関するレポート方法がまとめられている。

## テスト設計
- `tests/concurrency/thread_safety.rs` に以下のシナリオを実装予定
  - `summarize_with_details_is_thread_safe`: `KazeNhanhEngine` を `Arc` で共有しつつ 8 スレッド * 10 反復で要約処理を実行し、`HybridSummarizer` 内の `Mutex` ロック競合／デッドロックを監視
  - `git_native_rag_handles_parallel_invocations`: 各スレッドがテンポラリリポジトリを生成し `run_git_native_rag` を並行実行、`RepositoryContext` と `InferenceEngine` の共有ロックを検証
- スレッドは join 時に panic/エラーを伝搬させ、検出された不整合は即座にテスト失敗として報告

## 実行手順メモ
- 標準テスト: `cargo test concurrency::thread_safety`
- ThreadSanitizer 実行例 (nightly 必須):
  ```powershell
  $env:RUSTFLAGS="-Z sanitizer=thread"
  $env:RUSTDOCFLAGS="-Z sanitizer=thread"
  cargo +nightly test concurrency::thread_safety -- --nocapture
  ```
- ログ収集: `TSAN_OPTIONS="log_path=tsan"` を追加すると `tsan.*.log` にレース情報を出力可能

## 作業記録
- 2025-11-07 22:05 JST: `tests/concurrency/thread_safety.rs` を新規作成し、`summarize_with_details_is_thread_safe` シナリオを実装。
- 2025-11-07 22:30 JST: GitNativeRAG 並列実行テストを追加し、各スレッドでテンポラリリポジトリを生成して合成結果を検証。
- 2025-11-07 22:45 JST: ThreadSanitizer 実行手順を整備し、ログ収集オプションをドキュメント化。
- 2025-11-07 23:00 JST: `cargo test` / `cargo fmt` を実行し、新規テストが成功することを確認。
