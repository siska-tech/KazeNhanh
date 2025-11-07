---
status: completed
priority: medium
assignee: Backend
start_date: 2025-11-07
end_date: 2025-11-08
tags: ["testing", "quality"]
depends_on: ["task-pipeline-001-hybrid-summarizer", "task-pipeline-002-git-native-rag"]
---

# タスク概要
テスト仕様書に基づき、ユニット・結合・パフォーマンス・並行性テストのスイートとCI連携を整備する。

## 要件
- FRごとのユニットテスト（LexRank、GitService、MarkdownServiceなど）を`testing`フレームワークで作成し、カバレッジ目標C0 95%/C1 90%を満たす（`KZN-TEST-SPEC-001` 4章, 1.3）。
- 公開API向け結合テスト、`-race`付き並行性テスト、ベンチマークをCIに統合する（`KZN-TEST-SPEC-001` 1.2, 2.1）。
- 専用環境でのパフォーマンステスト・ソークテスト手順と指標収集の自動化を設計する（`KZN-TEST-SPEC-001` 5章）。

## サブタスク
- [x] [`subtask-testing-001-01-plan`](subtasks/subtask-testing-001-01-plan.md) テスト戦略分解とカバレッジ計画
- [x] [`subtask-testing-001-02-unit`](subtasks/subtask-testing-001-02-unit.md) 基盤・パイプライン向けユニットテスト実装
- [x] [`subtask-testing-001-03-integration`](subtasks/subtask-testing-001-03-integration.md) 公開API結合テストとサンプルデータ整備
- [x] [`subtask-testing-001-04-concurrency`](subtasks/subtask-testing-001-04-concurrency.md) 並行性テスト／`-race`シナリオの構築
- [x] [`subtask-testing-001-05-performance`](subtasks/subtask-testing-001-05-performance.md) パフォーマンス・ソークテスト環境とスクリプト準備
- [x] [`subtask-testing-001-06-ci`](subtasks/subtask-testing-001-06-ci.md) CI/CD統合とレポート自動化

