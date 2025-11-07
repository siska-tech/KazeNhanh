---
status: completed
priority: high
assignee: Backend
parent: task-foundation-001-nlp-service
start_date: 2025-11-07
end_date: 2025-11-07
tags: ["foundation", "nlp"]
depends_on: ["subtask-foundation-001-02-inmemory-init"]
---

# サブタスク概要
`NlpService::tokenize` を実装し、Sudachi の形態素情報を扱いやすい形で提供する。

## 完了条件
- Sudachi の `Morpheme` リストを取得できる。
- エラー時は `KazeNhanhError::TokenizationError` へ変換できる。
- 主要な出力（表層形や品詞など）をテストで確認している。

## 作業内容
- `src/foundation/nlp_service.rs` に `NlpService::tokenize` を追加し、`Mode::C` で Sudachi のトークン化を実行できるようにしました。
- Sudachi の `Morpheme` から UTF-8 文字列や各種メタ情報をコピーする `TokenizedMorpheme` を定義し、内部配列として `Vec<TokenizedMorpheme>` を返す設計に変更しました。
- `Tokenize` トレイトをインポートし、`StatelessTokenizer` の `tokenize` を呼び出せるように依存を明示しました。
- `tokenized_morpheme_accessors_return_expected_values` 単体テストを追加して、アクセサの戻り値と所有データの妥当性を確認しました。
- `cargo fmt` と `cargo test` を実行し、フォーマットとテストが成功することを確認しました。

