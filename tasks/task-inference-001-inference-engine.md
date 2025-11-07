---
status: completed
priority: high
assignee: Backend
start_date: 2025-11-07
end_date: 2025-11-07
tags: ["inference", "ai"]
depends_on: ["task-core-001-kaze-nhanh-engine"]
---

# タスク概要
candleベースの`InferenceEngine`と`KazeModel`実装を構築し、GGUFモデルをインメモリでロードしてテキスト合成を提供する。

## 要件
- `gguf_file::Content`と`VarBuilder::from_gguf_decompressed`でモデルをロードし、CPUデバイス強制を行う（`KZN-DETAIL-DESIGN-001` 4.2.1）。
- GGUFメタデータからトークナイザーを復元し、`Arc<Mutex<Tokenizer>>`で共有する（`KZN-DETAIL-DESIGN-001` 4.2.1）。
- `KazeModel::synthesize`でトークン生成ループを実装し、ログitsプロセッサやEOS検知、部分デコードを含める（`KZN-DETAIL-DESIGN-001` 4.2.2）。
- モデルロード・推論エラーをそれぞれ`ModelLoadError`と`ModelInferenceError`にマッピングする（`KZN-DETAIL-DESIGN-001` 4.6）。

## 進行状況
- 2025-11-07: `subtask-inference-001-05-error-handling` を完了し、推論ステージごとの詳細なエラーメッセージと `tracing::warn!` による復旧ログを追加。ストリーミングデコード失敗時の継続戦略と最終再デコード処理を整理。
- 2025-11-07: `subtask-inference-001-04-synthesize-loop` を完了し、`KazeModel::synthesize` にストリーミングデコードと EOS 判定の順序修正を加えてコンテキスト管理と部分デコードを設計通りに整備。
- 2025-11-07: `subtask-inference-001-03-constructor` を完了し、`KazeModel::ensure_cpu_device` でCPUデバイス強制を検証。`InferenceEngine::new` がCPU以外を拒否するようにし、`constructor_forces_cpu_device` テストでスモーク確認。
- 2025-11-07: `subtask-inference-001-02-tokenizer` を完了し、GGUFメタデータからのトークナイザー復元ロジックを共有モジュール化。復元フォールバックとスレッドセーフ性をユニットテスト (`tokenizer_is_shared_across_threads`) で検証。
- 2025-11-07: `subtask-inference-001-01-research` を完了し、Candle 0.9 系依存と CPU 強制構成を確認。`cargo check --release` で実機ビルドを検証し、GGUF 静的埋め込み・トークナイザー復元方針を文書化。
- 2025-11-07: `docs/inference_engine.md` に推論エンジンの利用手順とモック環境での初期レイテンシ基準値、ベンチマーク計画を整理。`subtask-inference-001-06-docs-bench` を完了し、README/ROADMAP を同期済み。

## サブタスク
- [x] [`subtask-inference-001-01-research`](subtasks/subtask-inference-001-01-research.md) Candle/GGUF対応状況と設計確認
- [x] [`subtask-inference-001-02-tokenizer`](subtasks/subtask-inference-001-02-tokenizer.md) トークナイザー復元と共有構造準備
- [x] [`subtask-inference-001-03-constructor`](subtasks/subtask-inference-001-03-constructor.md) `InferenceEngine::new`実装とデバイス設定
- [x] [`subtask-inference-001-04-synthesize-loop`](subtasks/subtask-inference-001-04-synthesize-loop.md) 推論ループとログits処理
- [x] [`subtask-inference-001-05-error-handling`](subtasks/subtask-inference-001-05-error-handling.md) エラーマッピングと回復戦略検討
- [x] [`subtask-inference-001-06-docs-bench`](subtasks/subtask-inference-001-06-docs-bench.md) ドキュメント整理とベンチマーク計画

