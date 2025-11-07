---
status: completed
priority: high
assignee: Backend
parent: task-core-001-kaze-nhanh-engine
start_date: 2025-11-07
end_date: 2025-11-07
tags: ["core", "initialization"]
depends_on: ["subtask-core-001-02-engine-config", "subtask-core-001-03-errors"]
---

# サブタスク概要
`KazeNhanhEngine::new`の実装を行い、NLPサービスと推論エンジンを初期化して内部フィールドに格納する。

## 完了条件
- `KazeNhanhEngine::new`が`Arc<Mutex<...>>`を用いてリソースを安全に保持する。
- NLP/Inference各モジュールの`new`呼び出しとエラーハンドリングが統合されている。
- 成功・失敗パスを確認するユニットテストまたはドキュメントが用意されている。

## 作業内容
- `KazeNhanhEngine` 構造体を実装し、`Arc<Mutex<InferenceEngine>>` と `Arc<NlpService>` を内部保持。
- Sudachi初期化失敗を `DictionaryLoadError` に、推論初期化失敗を `ModelLoadError` にマッピング。
- `inference` モジュールを追加し、モデルバイト長で初期化を検証する仮実装を配置。
- エンジン送受信性 (`Send + Sync`) と失敗パスを確認するユニットテストを追加。

## テスト
- `cargo test`

