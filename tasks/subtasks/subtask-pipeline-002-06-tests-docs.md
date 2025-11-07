---
status: completed
priority: medium
assignee: Backend
parent: task-pipeline-002-git-native-rag
start_date: 2025-11-07
end_date: 2025-11-07
tags: ["pipeline", "testing"]
depends_on: ["subtask-pipeline-002-05-fallbacks"]
---

# サブタスク概要
GitNativeRAGパイプラインのテストデータを準備し、シナリオテストとドキュメントを整備する。

## 完了条件
- 差分からレポート生成までの統合テストが追加されている。
- テスト用リポジトリやMarkdownサンプルが整備されている。
- ドキュメントに実行手順と期待結果が記載されている。

## 進捗メモ
- `docs/git_native_rag.md` を追加し、パイプラインの流れ・フォールバックシナリオ・`run_git_native_rag` の使用例を整理。
- `cargo test` で `pipeline::git_native_rag` のユニットテストを拡充済み（差分なしケース・推論成功ケースをカバー）。
- テストで利用する仮想リポジトリは `TempDir` ベースの既存ユニットテストで生成し、差分から報告生成までのシナリオを検証。

