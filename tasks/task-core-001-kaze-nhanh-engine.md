---
status: completed
priority: high
assignee: Backend
start_date: 2025-11-07
end_date: 2025-11-07
tags: ["api", "architecture"]
depends_on: []
---

# タスク概要
KazeNhanhライブラリの公開ファサードである`KazeNhanhEngine`と`EngineConfig`を実装し、設計書で定義されたリソース初期化、Tauri統合、エラーハンドリング要件を満たす。

## 要件
- `EngineConfig`がGGUFモデル・Sudachi辞書・設定ファイルを`&'static [u8]`として受け取り、ヘルパー`new`で初期化できるようにする（`KZN-API-SPEC-001` 2.1参照）。
- `KazeNhanhEngine::new`でNLPサービスと推論エンジンを初期化し、`Arc<Mutex<...>>`で内部保持する設計を実装する（`KZN-DETAIL-DESIGN-001` 4.4.1）。
- `KazeNhanhError`列挙体を`thiserror`で実装し、全レイヤーからのエラーを集約する（`KZN-API-SPEC-001` 2.2、表1）。
- Tauriステートで安全に共有できるよう`Send + Sync`制約を満たす内部構造を確保する（`KZN-API-SPEC-001` 原則0.1、`KZN-ARC-DESIGN-001` 4.2.1）。
- ドキュメントに記載された公開メソッド群（`generate_git_report`など）に署名を定義し、内部パイプラインへ委譲する準備を整える（`KZN-API-SPEC-001` 1.4, `KZN-DETAIL-DESIGN-001` 4.4.2-4.4.4）。

## サブタスク
- [x] [`subtask-core-001-01-spec-review`](subtasks/subtask-core-001-01-spec-review.md) 設計書の精査と不足要件の洗い出し
- [x] [`subtask-core-001-02-engine-config`](subtasks/subtask-core-001-02-engine-config.md) `EngineConfig`実装とバイトリソース受け渡し確認
- [x] [`subtask-core-001-03-errors`](subtasks/subtask-core-001-03-errors.md) `KazeNhanhError`定義と`thiserror`導入
- [x] [`subtask-core-001-04-engine-init`](subtasks/subtask-core-001-04-engine-init.md) `KazeNhanhEngine::new`の初期化ロジック実装
- [x] [`subtask-core-001-05-public-api`](subtasks/subtask-core-001-05-public-api.md) 公開メソッドのスタブと内部委譲の整備
- [x] [`subtask-core-001-06-docs`](subtasks/subtask-core-001-06-docs.md) README・タスクドキュメントの更新と整合性確認

## 進捗メモ (2025-11-07)
- 設計書レビューを完了し、`EngineConfig`・`KazeNhanhEngine`・`KazeNhanhError` の要件が実装状態と整合していることを確認。GGUF/Sudachi リソースの詳細検証は推論タスクで補完予定。
- 公開API（`generate_git_report` / `summarize_document` など）を実装し、Git差分集計・簡易要約・重要文抽出・Sudachiトークナイズを内部サービスへ委譲する土台を整備。
- README・API仕様に公開メソッドの利用方法と暫定実装メモを追記し、ROADMAP/タスク進捗を更新。

## 完了サマリ
- `EngineConfig`・`KazeNhanhEngine::new`・公開API・ドキュメント更新まで、タスク要件をすべて満たし終結。今後は推論パイプライン・LexRank 実装で暫定処理を置き換える。

