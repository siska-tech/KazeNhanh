---
status: completed
priority: medium
assignee: Backend
parent: task-testing-001-quality-assurance
start_date: 2025-11-07
end_date: 2025-11-08
tags: ["testing", "ci"]
depends_on: ["subtask-testing-001-05-performance"]
---

# サブタスク概要
CI/CDパイプラインにテストスイートを統合し、レポート生成や失敗時のアラートを自動化する。

## 完了条件
- CI設定ファイルにユニット・結合・並行性・パフォーマンス試験が組み込まれている。
- カバレッジレポートや性能結果が自動で集計・保存される。
- 失敗時の通知ルールが設定されている。

## CIパイプライン設計
- Workflow: `.github/workflows/testing.yml`
  - `tests` ジョブ: `cargo fmt --check` / `cargo test --all --all-features` 実行後 `cargo llvm-cov` で `coverage/lcov.info` を生成しアーティファクト化。
  - `thread-sanitizer` ジョブ: nightly + `RUSTFLAGS=-Z sanitizer=thread` で並行性テスト (`summarize_with_details_is_thread_safe`, `git_native_rag_handles_parallel_invocations`) を1スレッドで実行し競合検知。
  - `performance` ジョブ: `cargo bench --features mock_inference --bench performance` による Criterion 計測と `scripts/soak/run_soak.ps1` (5分設定) による短時間ソークを実行し、結果をアーティファクトに保存。
- 失敗時通知: GitHub Checks ステータスでPR/ブランチに即時反映。アーティファクト `coverage-lcov` / `criterion-results` / `soak-logs` で詳細を参照可能。

## KPIとレポート
- カバレッジ: `cargo llvm-cov` (LCOV) を生成し、CI上でダウンロード可能にして外部サービス(Codecov 等)に連携可能。
- パフォーマンス: Criterion JSON/HTML レポート (`artifacts/criterion`) を保存し、コミット間比較に利用。
- ソーク: `artifacts/soak/<timestamp>` に各イテレーションのログとベンチ結果を集約。

## 実行手順メモ
- ローカル検証
  ```powershell
  cargo llvm-cov --workspace --html
  cargo bench --features mock_inference --bench performance
  pwsh ./scripts/soak/run_soak.ps1 -DurationMinutes 5 -IntervalSeconds 60
  ```
- CI 手動再実行は GitHub Actions 画面から対象ジョブを再実行。

## 作業記録
- 2025-11-08 00:05 JST: `.github/workflows/testing.yml` を作成し、テスト / ThreadSanitizer / ベンチマークの3ジョブ構成を定義。
- 2025-11-08 00:20 JST: パフォーマンスジョブで Criterion/ソーク結果をアーティファクト化し、mock_inference フィーチャ経由でベンチを安定化。
- 2025-11-08 00:30 JST: サブタスク文書へ KPI/手順/作業記録を追記し、ローカルで `cargo test` / `cargo bench --features mock_inference` を実行確認。

