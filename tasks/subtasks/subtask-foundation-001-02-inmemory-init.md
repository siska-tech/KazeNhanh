---
status: completed
priority: high
assignee: Backend
parent: task-foundation-001-nlp-service
start_date: 2025-11-07
end_date: 2025-11-07
tags: ["foundation", "nlp"]
depends_on: ["subtask-foundation-001-01-spec-review"]
---

# サブタスク概要
Sudachi v0.6.9以降のインメモリ辞書ロードAPIを利用し、`NlpService::new`で`EngineConfig`のバイト列からトークナイザーを構築する。

## 完了条件
- `NlpService::new`が静的バイト列を用いてTokenizerを生成できる。
- エラーケース（UTF-8不正、辞書不整合等）がハンドリングされている。
- 最低限のユニットテストまたはモックで初期化成功が確認されている。

## 実施内容
- `sudachi` gitタグ `v0.6.9` を依存に追加し、`ConfigBuilder::from_bytes` と `JapaneseDictionary::from_cfg_storage_with_embedded_chardef` を使用してインメモリ初期化を実装。
- `catch_unwind` を用いてSudachi内部パニックを`SudachiError::InvalidDictionaryGrammar`に変換。
- 異常系ユニットテストを追加し、`cargo test`/`cargo fmt` を実行して正常動作を確認。

