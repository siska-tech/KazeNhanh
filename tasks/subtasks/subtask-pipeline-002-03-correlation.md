---
status: completed
priority: high
assignee: Backend
parent: task-pipeline-002-git-native-rag
start_date: 2025-11-07
end_date: 2025-11-07
tags: ["pipeline", "rag"]
depends_on: ["subtask-pipeline-002-02-diff-ingest"]
---

# サブタスク概要
`correlation_engine::correlate`を実装し、追加行とMarkdownセクションを突合して`ContextualChange`リストを生成する。

## 完了条件
- 行番号範囲判定が正しく実装され、適切なセクション名が紐付く。
- セクションが見つからない場合のデフォルト処理が定義されている。
- 代表的な差分パターンを用いたテストが追加されている。

## 進捗メモ
- `pipeline/correlation_engine.rs` を追加し、`ContextualChange`構造体と `correlate` 関数で `DiffIngestionEntry` の追加行をMarkdownセクション単位にグルーピング。
- 見出しマッチ・未分類フォールバック・同一セクション複数行のケースをカバーするユニットテストを実装。
- セクション未検出時はタイトルなしクラスタにまとめるデフォルトキーを定義し、後続プロンプト生成で扱いやすい形式に整形。

