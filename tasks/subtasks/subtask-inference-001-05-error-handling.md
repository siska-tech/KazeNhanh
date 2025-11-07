---
status: completed
priority: medium
assignee: Backend
parent: task-inference-001-inference-engine
start_date: 2025-11-07
end_date: 2025-11-07
tags: ["inference", "error-handling"]
depends_on: ["subtask-inference-001-04-synthesize-loop"]
---

# サブタスク概要
モデルロード・推論時のエラーケースを洗い出し、`ModelLoadError`と`ModelInferenceError`を使った例外処理を統合する。

## 完了条件
- 想定される失敗（モデル不一致、推論中断等）に対するエラーが実装されている。
- エラー発生時のログ出力や再試行戦略が検討されている。
- テストまたはモックでエラーパスが検証されている。

## 進捗メモ
- モデルロード失敗・推論失敗の双方で`CandleError`を捕捉し、`ModelLoadError`/`ModelInferenceError`にマッピング。
- Mutexロックやトークナイザー復元失敗時も`CandleError::Msg`経由で整形。
- モック実装で故意にエラーを発生させる単体テスト (`synthesize_maps_candle_error`) を追加しハンドリングを検証。

## 進行状況
- 2025-11-07: 推論ステージごとの `stage_error` を追加し、トークナイザー操作・テンソル生成・モデル forward・サンプリング・デコードの各ポイントで詳細な `CandleError` メッセージを付与。ストリーミングデコード失敗時には `tracing::warn!` でログ出力しつつ処理継続、最終デコードで再試行し、すべてのエラーが `ModelLoadError`/`ModelInferenceError` に帰着することを確認。

