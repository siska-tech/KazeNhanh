---
status: completed
priority: high
assignee: Backend
parent: task-inference-001-inference-engine
start_date: 2025-11-07
end_date: 2025-11-07
tags: ["inference", "ai"]
depends_on: ["subtask-inference-001-03-constructor"]
---

# サブタスク概要
`KazeModel::synthesize`の推論ループを実装し、ログitsプロセッサによるトークン生成とEOS判定、部分デコードを行う。

## 完了条件
- トークン生成ループが最大トークン数・コンテキスト長を尊重して動作する。
- EOS判定とログitsプロセッサ設定が設計通りになっている。
- 推論結果が人間可読な文字列として返り、テストで検証されている。

## 進捗メモ
- `LogitsProcessor::new` を用いた Top-p + Temperature サンプリングを実装し、固定シードで再現性を確保。
- コンテキストはスライディングウィンドウで管理し、`ModelWeights` を都度クローンして KV キャッシュを初期化。
- EOS トークンはメタデータから判別しつつ既定候補 (`</s>` 等) をフォールバックするロジックを追加。

## 進行状況
- 2025-11-07: 推論ループで EOS トークン検知をサンプリング直後に行い、コンテキストと生成トークンへの追加を制御。`LogitsProcessor` による Top-p/Temperature サンプリングを保持しつつ、生成トークンを用いたストリーミングデコード（部分デコード）を実装し、UTF-8 境界を評価しながら最終出力を構築するように更新。

