---
status: completed
priority: high
assignee: Backend
parent: task-inference-001-inference-engine
start_date: 2025-11-07
end_date: 2025-11-07
tags: ["inference", "ai"]
depends_on: ["subtask-inference-001-01-research"]
---

# サブタスク概要
GGUFメタデータからトークナイザーを復元し、`Arc<Mutex<Tokenizer>>`で共有できるように実装する。

## 完了条件
- GGUFから取得したトークナイザーデータを`tokenizers::Tokenizer`へ復元できる。
- 共有構造体のスレッドセーフ性を検証するテストまたは静的解析がある。
- 復元失敗時に適切な`ModelLoadError`が返る。

## 進捗メモ
- `candle_core::quantized::gguf_file::Content`のメタデータから`tokenizer.json`もしくはu8配列を探索し、復元処理を実装。
- 復元できない場合は`WordLevel`ベースのフォールバックを構築して`Arc<Mutex<Tokenizer>>`で共有。
- Mutexロック失敗時は`CandleError::Msg`へマッピングし、上位で`ModelLoadError`として取り扱う。

## 進行状況
- 2025-11-07: トークナイザー復元ロジックを `tokenizer_support` モジュールとして分離し、GGUFメタデータからの復元とフォールバック構築をユニットテストで検証。`Arc<Mutex<Tokenizer>>` の `Send`/`Sync` 特性とスレッド共有テスト (`tokenizer_is_shared_across_threads`) を追加し、`InferenceEngine::tokenizer` の安全性を確認。

