---
status: completed
priority: medium
assignee: Backend
parent: task-foundation-002-markdown-service
start_date: 2025-11-07
end_date: 2025-11-07
tags: ["foundation", "markdown"]
depends_on: ["subtask-foundation-002-01-parser-options"]
---

# サブタスク概要
行頭オフセットを高速に引ける`LineOffsetMap`ヘルパーを実装し、バイトオフセットから行番号への変換を可能にする。

## 完了条件
- `LineOffsetMap`構造体が実装され、二分探索による行番号解決が可能になっている。
- 長文入力や末尾の改行有無などのケースをテストで確認している。
- APIコメントに利用例と計算量の注意点が記載されている。

## 実装メモ
- `line_for_offset`はオフセット境界を二分探索し、オフセットが文字列長を超えた場合は`MarkdownParseError`を返す。
- `total_lines`は最終バイト位置より前の行頭のみを数えるため、末尾改行による空行を重複カウントしない。
- 単行・空文字・複数連続改行を扱うユニットテストを追加し、境界ケースを検証済み。

