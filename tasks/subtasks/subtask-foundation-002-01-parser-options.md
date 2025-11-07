---
status: completed
priority: medium
assignee: Backend
parent: task-foundation-002-markdown-service
start_date: 2025-11-07
end_date: 2025-11-07
tags: ["foundation", "markdown"]
depends_on: []
---

# サブタスク概要
pulldown-cmarkのオプションとイベントフローを整理し、行番号取得に必要な設定・前処理を設計する。

## 完了条件
- `Parser::into_offset_iter`に基づくソース位置取得方法が明記された設計メモがある。
- Headingイベントの扱いとテキスト抽出手順が定義されている。
- 工数見積もりや潜在的なエッジケースがリストアップされている。

## 実装メモ
- pulldown-cmark v0.13では`Options::ENABLE_SOURCEPOS`が廃止されたため、`Options::empty()`構成で`into_offset_iter`を利用してバイトレンジを取得する方針に更新。
- Heading開始イベントで既存セクションを確定させ、終了イベントでタイトル集約と`HeadingLevel`整合性チェックを行う。
- ソフト・ハードブレークはスペースへ正規化し、HTML/コード断片は生文字列として連結してタイトルを構成する。

