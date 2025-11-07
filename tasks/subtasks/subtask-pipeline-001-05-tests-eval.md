---
status: completed
priority: medium
assignee: Backend
parent: task-pipeline-001-hybrid-summarizer
start_date: 2025-11-07
end_date: 2025-11-07
tags: ["pipeline", "testing"]
depends_on: ["subtask-pipeline-001-04-orchestrator"]
---

# サブタスク概要
HybridSummarizerのテスト・評価ベンチを作成し、品質指標（ROUGEなど）の計測計画を立てる。

## 完了条件
- 抽出・合成それぞれのユニットテストと統合テストが整備されている。
- 簡易ベンチマークまたは評価スクリプトの雛形が用意されている。
- テスト実行手順と期待成果がドキュメント化されている。

## テスト整備メモ
- `src/pipeline/sentence.rs` に文分割・品詞フィルタリング・BoW生成のユニットテストを追加済み。
- `src/pipeline/lex_rank_engine.rs` にコサイン類似度とPageRank収束を検証するユニットテストを追加済み。
- `src/pipeline/hybrid_summarizer.rs` に抽出+合成を跨ぐ統合テスト（正常系／LexRank非収束）を追加済み。
- `scripts/hybrid_eval_template.rs` で生成するJSONLを後続のROUGE計測に利用できる。
- 実行コマンド: `cargo test --package kaze_nhanh --lib pipeline`（全テストの場合は`cargo test`）。

## 評価計画
- JSONL出力: `scripts/hybrid_eval_template.rs` をベースに、`id\ttext` 形式のコーパスから抽出文と要約文を出力。
- 品質指標: 外部ツール（例: `py-rouge` / `rouge-score`）でROUGE-1/ROUGE-Lを計測。抽出文ベースと合成本文ベースの双方を比較し、Drip（抽出）/Stir（合成）の両品質を可視化。
- スケジュール: デイリー回帰として10件のサンプリング評価を自動化、週次で全量評価を実施。閾値: ROUGE-L >= 0.28 を暫定合格ラインとする。
- 手順メモ:
  1. `scripts/hybrid_eval_template.rs` を本番バイナリに組み込み、実際のGGUF/Sudachiバイト列を設定。
  2. `cargo run --example hybrid_eval --release` 等でJSONLを生成。
  3. PythonスクリプトでROUGEレポートを出力し、`artifacts/` 配下に集約する。

