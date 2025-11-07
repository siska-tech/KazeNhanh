---
status: completed
priority: high
assignee: Backend
start_date: 2025-11-07
end_date: 2025-11-07
tags: ["foundation", "nlp"]
depends_on: ["task-core-001-kaze-nhanh-engine"]
---

# タスク概要
Sudachi辞書をインメモリでロードする`NlpService`と文分割関数`split_sentences`を実装し、日本語NLP基盤（FR1）を提供する。

## 要件
- `EngineConfig`から受け取った`dictionary_bytes`と`settings_bytes`を用いてSudachiトークナイザーを初期化する（`KZN-DETAIL-DESIGN-001` 4.1.1）。
- Sudachi v0.6.9以降のインメモリ辞書ロード機能を利用し、ファイルパスに依存しない構築を行う（`KZN-API-SPEC-001` 原則0.2）。
- 形態素解析`tokenize`と文分割`split_sentences`を`pub(crate)`で提供し、L2レイヤーから利用できるようにする（`KZN-DETAIL-DESIGN-001` 4.1.1, `KZN-REQ-SPEC-001` 3.1）。
- Sudachiエラーを`KazeNhanhError::DictionaryLoadError`および`TokenizationError`へマッピングする（`KZN-DETAIL-DESIGN-001` 4.6, 表4.2）。

## サブタスク
- [x] [`subtask-foundation-001-01-spec-review`](subtasks/subtask-foundation-001-01-spec-review.md) Sudachi設定とAPI要件の精査
- [x] [`subtask-foundation-001-02-inmemory-init`](subtasks/subtask-foundation-001-02-inmemory-init.md) インメモリ辞書ロード`NlpService::new`実装
- [x] [`subtask-foundation-001-03-tokenize`](subtasks/subtask-foundation-001-03-tokenize.md) `tokenize`メソッド実装とエラーマッピング
- [x] [`subtask-foundation-001-04-split-sentences`](subtasks/subtask-foundation-001-04-split-sentences.md) 文分割関数と公開モジュール化
- [x] [`subtask-foundation-001-05-tests-docs`](subtasks/subtask-foundation-001-05-tests-docs.md) 単体テスト整備とドキュメント更新

## 実施内容
- Sudachi設定・辞書のインメモリ初期化を実装し、`NlpService` を通じてトークナイズ機能を提供しました。
- `split_sentences` を `foundation::nlp` から再エクスポートし、sakuベースの文分割と追加ヒューリスティックを導入しました。
- エラーマッピング・テスト・READMEドキュメントを整備し、既知の制約と利用例を明記しました。

