---
status: completed
priority: medium
assignee: Backend
parent: task-pipeline-001-hybrid-summarizer
start_date: 2025-11-07
end_date: 2025-11-07
tags: ["pipeline", "planning"]
depends_on: []
---

# サブタスク概要
HybridSummarizerの全体フローと設定値（抽出文数、閾値等）を整理し、依存モジュールとの連携仕様を明確化する。

## 完了条件
- `count`パラメータや閾値のデフォルト案が決定されている。
- NLPサービス・推論エンジンとのインターフェースが仕様化されている。
- エラーハンドリング方針とリトライ条件が定義されている。

## 設計方針
- ハイブリッド要約は抽出ステージと合成ステージを分離しつつ、`HybridSummarizer::execute`で一体として扱う。
- 抽出ステージではLexRankスコアで重要文を選抜し、合成ステージで`InferenceEngine::synthesize`を呼び出して要約本文を生成する。
- `HybridSummary`構造体（仮名）で抽出済み文と合成済みテキストを返却し、クライアントは用途に応じて使い分けられるようにする。

## デフォルトパラメータ案
- `max_sentences`: 5 — FR3のDrip要約要件に合わせて短い抜粋を維持する。
- `similarity_threshold`: 0.12 — PageRankスコアの下限。長文でのノイズを除去しつつ、短文でも最低1文は残る値。
- `damping_factor`: 0.85 — 一般的なLexRank設定値を採用し、スコア変動の安定性を確保する。
- `max_iterations`: 50 — 100文規模でも収束を期待できる反復上限。
- `convergence_delta`: 1e-4 — PageRank収束判定用の閾値。
- `min_characters`: 25 — 極端に短い文を抽出候補から除外する下限。
- `max_token_frequency`: 6 — TF-IDF化の際に高頻度語を抑制するヒューリスティック。

## インターフェース仕様（案）
- `HybridSummarizer::new(nlp: Arc<NlpService>, inference: Arc<Mutex<InferenceEngine>>, config: HybridSummarizerConfig)`
- `HybridSummarizer::execute(&self, text: &str) -> Result<HybridSummary, KazeNhanhError>`
- `HybridSummary`は`extracted_sentences: Vec<String>`と`synthesized_summary: Option<String>`を保持し、`KazeNhanhEngine::summarize_document`では`synthesized_summary`を返す。
- `lex_rank_engine::rank_sentences(sentences: &[SentenceEmbedding], options: &LexRankOptions) -> Result<Vec<RankedSentence>, SummarizeEngineError>`を公開し、`HybridSummarizer`は`position`フィールドで原順序を復元する。
- `SentenceEmbedding`は元文（`text`）、位置（`position`）、正規化表層形のトークン（`tokens: Vec<String>`）を保持する。

## エラーハンドリングとリトライ
- Sudachiトークナイズ失敗時は即座に`KazeNhanhError::TokenizationError`として伝搬する（再試行なし）。
- LexRank計算中に発生するゼロ除算や不収束は`KazeNhanhError::SummarizeEngineError`で包む。
- 推論エンジン呼び出しは既存の`InferenceEngine::synthesize`を使用し、失敗時は`ModelInferenceError`をそのまま返す。将来的なリトライ余地として`HybridSummarizerConfig`に`retry_on_failure`を追加で保持できるようにしておく。
- 抽出結果が空の場合は`split_sentences`の先頭1文をフォールバックとして利用し、それでも抽出できない場合は`SummarizeEngineError("lexrank produced no candidates")`を返す。

## テスト観点メモ
- 最小入力（空文字、1文のみ）としきい値前後のケースで動作確認する。
- PageRankが収束するケースと閾値で弾かれるケースの双方をカバーする。
- Sudachiトークナイズ失敗をモック化し、`TokenizationError`へのマッピングを検証する。
- 高スコア文が原文順では逆順になるケースを用意し、復元処理で原順序が保持されるかを確認する。

