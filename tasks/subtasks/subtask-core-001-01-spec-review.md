---
status: completed
priority: high
assignee: Backend
parent: task-core-001-kaze-nhanh-engine
start_date: 2025-11-07
end_date: 2025-11-07
tags: ["core", "analysis"]
depends_on: []
---

# サブタスク概要
API設計書と詳細設計書を再確認し、`KazeNhanhEngine`まわりの仕様・制約・未解決事項を整理する。

## 完了条件
- 参照ドキュメントの該当箇所に対する読み合わせメモが作成されている。
- 追加で必要な設定値や外部依存が洗い出され、親タスクに反映されている。
- 想定するユースケースとエラーシナリオが一覧化されている。

## レビュー結果サマリ
- `EngineConfig::new` は `model_bytes` / `dictionary_bytes` / `settings_bytes` を `'static [u8]` として受け取り、`include_bytes!` でバンドルするゼロセットアップ方針を明示している（KZN-API-SPEC-001 §0.2, §2.1）。
- `KazeNhanhEngine::new` は `NlpService::new` と `InferenceEngine::new` の初期化手順を段階化し、`Arc<Mutex<InferenceEngine>>` と `Arc<NlpService>` でラップして `Send + Sync` を満たす設計になっている（KZN-DETAIL-DESIGN-001 §4.4.1）。
- `KazeNhanhError` は `thiserror` による共通エラーラッピングで、`candle_core::Error` / `sudachi::Error` / `std::io::Error` などを `map_err` で親に伝搬する仕様が定義済み（KZN-API-SPEC-001 §0.3, KZN-DETAIL-DESIGN-001 §4.4）。
- Tauri との結合は `tauri::Builder::manage(engine)` を前提に、`State<KazeNhanhEngine>` からの多スレッドアクセス要件が両仕様書に反映されている（KZN-API-SPEC-001 §0.1, KZN-DETAIL-DESIGN-001 §4.4）。

## 追加で確認が必要な事項
- `include_bytes!` で読み込む GGUF / Sudachi リソースの配置パスとサイズ見積もりがまだ未決。`EngineConfig` 実装時にサンプルパスを明文化する。
- `InferenceEngine::new` が要求する GGUF のバージョン互換とファイル検証手順を `docs` 配下の補足資料から再確認し、読み込み時のバリデーション要件を明確化する。
- エラーラッピングのテスト戦略（`map_err` 経由での `KazeNhanhError` 変換）が CI で担保されるか、ユニットテスト方針を次のサブタスクで検討する。

## 次アクション
- `subtask-core-001-02-engine-config` で `EngineConfig` 構造体と `include_bytes!` の配置を具体化し、親タスクの要件と差分が出ないか確認する。
- Sudachi / GGUF アセットの取得・配置に関する補足ドキュメントを作成し、`README` あるいは `docs` への反映案をまとめる。

## 追加メモ
- 仕様差分なし。`EngineConfig`/`KazeNhanhError` 要件が実装済みであることを確認。
- GGUF/Sudachi リソースのバリデーション手順は `task-inference-001` 着手時に補完する。

