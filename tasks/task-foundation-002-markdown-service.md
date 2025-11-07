---
status: completed
priority: medium
assignee: Backend
start_date: 2025-11-07
end_date: 2025-11-07
tags: ["foundation", "markdown"]
depends_on: ["task-core-001-kaze-nhanh-engine"]
---

# タスク概要
Pulldown-cmarkを用いてMarkdown構造と行番号マッピングを返す`MarkdownService`を実装し、GitネイティブRAGの文脈特定を支える。

## 要件
- `Parser::into_offset_iter`を用いたパーサで見出しイベントを巡回し、`MarkdownSection`のリストを生成する（`KZN-DETAIL-DESIGN-001` 4.1.2）。
- バイトオフセットを行番号へ変換する`LineOffsetMap`ヘルパーを実装し、FR2.2の精度要件を満たす（`KZN-DETAIL-DESIGN-001` 4.1.2、`KZN-REQ-SPEC-001` 3.2）。
- 解析エラー時に`KazeNhanhError::MarkdownParseError`を返す（`KZN-DETAIL-DESIGN-001` 4.6）。

## サブタスク
- [x] [`subtask-foundation-002-01-parser-options`](subtasks/subtask-foundation-002-01-parser-options.md) パーサ設定とイベント処理方針の整理
- [x] [`subtask-foundation-002-02-line-offset-map`](subtasks/subtask-foundation-002-02-line-offset-map.md) `LineOffsetMap`ヘルパー実装
- [x] [`subtask-foundation-002-03-structure-mapping`](subtasks/subtask-foundation-002-03-structure-mapping.md) `map_document_structure`実装とテスト
- [x] [`subtask-foundation-002-04-error-handling`](subtasks/subtask-foundation-002-04-error-handling.md) エッジケース対応とエラーハンドリング
- [x] [`subtask-foundation-002-05-docs-examples`](subtasks/subtask-foundation-002-05-docs-examples.md) ドキュメント／使用例追加

