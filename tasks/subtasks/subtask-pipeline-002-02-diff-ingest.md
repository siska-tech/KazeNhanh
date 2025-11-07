---
status: completed
priority: high
assignee: Backend
parent: task-pipeline-002-git-native-rag
start_date: 2025-11-07
end_date: 2025-11-07
tags: ["pipeline", "rag"]
depends_on: ["subtask-pipeline-002-01-design"]
---

# サブタスク概要
GitサービスとMarkdownサービスを統合し、差分取得から構造解析までの前処理パイプラインを実装する。

## 完了条件
- `GitService`と`MarkdownService`を呼び出してコンテキスト情報が取得できる。
- ファイル単位のループで必要なデータが集約される。
- 入力が存在しない場合のフォールバックが実装されている。

## 進捗メモ
- `GitNativeRAG::ingest_diffs` を実装し、`prepare_repository`→`collect_markdown_diffs`→Markdown構造解析のパイプラインを確立。
- 差分と見出し情報を保持する `DiffIngestionEntry` / `DiffIngestionResult` を定義し、差分なしの場合は空結果を返すフォールバックを用意。
- Tempリポジトリを構築するユニットテストで、差分あり/なし双方のケースを検証。

