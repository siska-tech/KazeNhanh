---
status: completed
priority: medium
assignee: Backend
parent: task-foundation-002-markdown-service
start_date: 2025-11-07
end_date: 2025-11-07
tags: ["foundation", "markdown"]
depends_on: ["subtask-foundation-002-03-structure-mapping"]
---

# サブタスク概要
Markdown解析時の異常系を洗い出し、`KazeNhanhError::MarkdownParseError`への変換やフォールバック戦略を整理する。

## 完了条件
- 入力不正・未知イベント・行番号計算失敗などのケースで適切なエラーを返す。
- ロギングやトレース用の情報がErrorメッセージに含まれている。
- エラーハンドリングを検証するテストが追加されている。

## 実装メモ
- バイトオフセットが文字列長を超えた場合や無効なレンジに対して`MarkdownParseError`を返し、メッセージにオフセット値を含めてデバッグ性を確保。
- 見出しの開始・終了イベントでレベルが整合しない場合はエラーとして早期リターンし、壊れたMarkdown構造を検出。
- 行番号境界・異常イベントはユニットテストで検証し、異常ケースがpanicではなく`Result`で上位に伝播することを確認。

