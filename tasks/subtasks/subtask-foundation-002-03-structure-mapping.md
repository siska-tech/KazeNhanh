---
status: completed
priority: high
assignee: Backend
parent: task-foundation-002-markdown-service
start_date: 2025-11-07
end_date: 2025-11-07
tags: ["foundation", "markdown"]
depends_on: ["subtask-foundation-002-02-line-offset-map"]
---

# サブタスク概要
`map_document_structure`関数を実装し、見出しイベントから`MarkdownSection`のリストを生成して返す。

## 完了条件
- 見出しレベル・テキスト・行範囲が正しくマッピングされる。
- 入力Markdownに対する代表的なテストケースが作成されている。
- ネスト構造や同レベル見出し連続などのケースで期待通りの範囲が算出される。

## 実装メモ
- `MarkdownService::map_document_structure`は見出し開始時に既存セクションを確定し、終了時にタイトル正規化と`HeadingLevel`整合性検証を行う。
- セクションの終端は次見出し直前まで延長し、最終セクションはドキュメントの有効行数まで拡張することでコンテンツ行を包含。
- インラインコード・HTML片はスペース区切りでタイトルに連結し、マークダウンのブレークイベントは単一スペースに畳み込む。

