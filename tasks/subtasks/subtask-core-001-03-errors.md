---
status: completed
priority: high
assignee: Backend
parent: task-core-001-kaze-nhanh-engine
start_date: 2025-11-07
end_date: 2025-11-07
tags: ["core", "error-handling"]
depends_on: ["subtask-core-001-01-spec-review"]
---

# サブタスク概要
`thiserror`を用いた`KazeNhanhError`列挙体を定義し、下位レイヤーのエラーを網羅的にラップできるようにする。

## 完了条件
- 設計書で列挙された全バリアントが`KazeNhanhError`に定義されている。
- `#[from]`や`#[source]`属性を利用し、エラー伝播が自動的に機能する。
- サンプルコードまたはテストで`?`演算子による変換が確認されている。

## 作業内容
- 設計書の表1に従って`KazeNhanhError`の各バリアント（Git操作・Sudachi・Candle・Markdown・Diffなど）と`#[source]`/`#[from]`属性を実装。
- `candle-core`依存関係を追加し、`ModelLoadError`/`ModelInferenceError`のソース型を定義。
- `GitService`テストを更新し、`GitOperationError`の`source`メッセージを検証。
- エラー変換を検証する単体テストを追加し、`?`演算子経由で`std::io::Error`やSudachi/Candleエラーが伝播することを確認。

## テスト
- `cargo test`

