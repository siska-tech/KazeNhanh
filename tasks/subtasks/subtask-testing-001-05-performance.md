---
status: completed
priority: medium
assignee: Backend
parent: task-testing-001-quality-assurance
start_date: 2025-11-07
end_date: 2025-11-07
tags: ["testing", "performance"]
depends_on: ["subtask-testing-001-04-concurrency"]
---

# サブタスク概要
パフォーマンスおよびソークテストの環境とスクリプトを準備し、KPI測定手順を確立する。

## 完了条件
- 負荷ツールやカスタムクライアントの設定が完了している。
- KPI測定項目と閾値が自動判定できるようになっている。
- 実行手順およびログ取得方法がドキュメント化されている。

## テスト設計
- ベンチマーク: `benches/performance.rs`
  - `summaries/hybrid`: `summarize_with_details` のレイテンシ測定。Criterion を用い、入力テキストサイズに対するスループット (bytes/sec) を記録。
  - `git_native_rag/ingest_and_summarize`: テンポラリリポジトリを生成し `run_git_native_rag` をベンチマーク。差分抽出と合成の総レイテンシを計測。
- ソーク: `scripts/soak/run_soak.ps1`
  - `cargo bench --bench performance` を一定間隔で繰り返し、Criterion の JSON/HTML レポートを `artifacts/soak/<timestamp>` に保存。
  - 失敗時は直近ログ (`bench-*.json`) と PowerShell exit code を CI にアップロードする。

## KPIと閾値 (ドラフト)
- `summaries/hybrid`
  - p95 レイテンシ ≤ 25ms
  - スループット ≥ 40kB/s
- `git_native_rag/ingest_and_summarize`
  - p95 レイテンシ ≤ 60ms
  - 失敗率 0、生成レポートが常に `Synthesized` を返すこと
- ソークテスト
  - 30分連続実行中の失敗なし
  - CPU平均使用率 ≤ 70%、メモリ常用量 ≤ +150MB (baseline 比)

## 実行手順メモ
- ベンチマーク: `cargo bench --features mock_inference --bench performance`
- ソークテスト (30分例):
  ```powershell
  pwsh .\scripts\soak\run_soak.ps1 -DurationMinutes 30 -IntervalSeconds 120
  ```
- 環境変数 `CRITERION_OUTPUT` を指定すると、HTMLレポートが指定ディレクトリに保存される。

## 作業記録
- 2025-11-07 23:20 JST: `benches/performance.rs` を追加し、Criterion ベースでハイブリッド要約と GitNativeRAG パイプラインを計測するベンチマークを実装。
- 2025-11-07 23:35 JST: `src/lib.rs` に `bench_support` モジュールを実装し、モック推論と簡易トークナイザーでベンチ用途の補助関数を提供。
- 2025-11-07 23:45 JST: `scripts/soak/run_soak.ps1` を作成し、ベンチ実行のループとアーティファクト収集を自動化。
- 2025-11-07 23:55 JST: `cargo bench --features mock_inference --bench performance` を実行し、Criterion レポート生成を確認。

