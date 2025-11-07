---
status: completed
priority: medium
assignee: Backend
start_date: 2025-11-07
end_date: 2025-11-07
tags: ["pipeline", "summarization"]
depends_on: ["task-foundation-001-nlp-service", "task-inference-001-inference-engine"]
---

# タスク概要
HybridSummarizerパイプラインとLexRankEngineを実装し、FR3のDrip & Stir要約を提供する。

## 要件
- `HybridSummarizer::execute`で抽出ステージと合成ステージを連結し、`KazeNhanhEngine::summarize_document`へ提供する（`KZN-DETAIL-DESIGN-001` 4.3.1）。
- `lex_rank_engine::rank_sentences`を実装し、Sudachiで形態素解析した文を元にコサイン類似度行列とPageRankを計算する（`KZN-DETAIL-DESIGN-001` 4.3.2、`KZN-REQ-SPEC-001` 3.3.1）。
- 重要文抽出結果の順序復元、閾値設定、エラーマッピング（`SummarizeEngineError`/`TokenizationError`）を行う（`KZN-DETAIL-DESIGN-001` 4.3.2, 4.6）。

## サブタスク
- [x] [`subtask-pipeline-001-01-design`](subtasks/subtask-pipeline-001-01-design.md) パイプライン構成と設定値の精査
- [x] [`subtask-pipeline-001-02-input-prep`](subtasks/subtask-pipeline-001-02-input-prep.md) 文分割と形態素解析の連携実装
- [x] [`subtask-pipeline-001-03-lexrank`](subtasks/subtask-pipeline-001-03-lexrank.md) 類似度行列とPageRank計算の実装
- [x] [`subtask-pipeline-001-04-orchestrator`](subtasks/subtask-pipeline-001-04-orchestrator.md) `HybridSummarizer::execute`オーケストレーション
- [x] [`subtask-pipeline-001-05-tests-eval`](subtasks/subtask-pipeline-001-05-tests-eval.md) テスト・評価ベンチとドキュメント更新

