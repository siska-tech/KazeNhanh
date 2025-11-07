# Inference Engine Usage & Benchmark Plan

## 概要
- `KazeModel` は Candle の量子化 LLaMA ウェイトを GGUF 形式で読み込み、CPU 専用デバイス (`Device::Cpu`) で推論を行います。
- `InferenceEngine` は `EngineConfig` に含まれる GGUF モデル・Sudachi 辞書・設定 JSON を受け取り、トークナイザー復元と推論ループを初期化します。
- パブリック API からは `KazeNhanhEngine` を通じて推論を呼び出す設計になっており、内部で同一の `InferenceEngine` を共有します。

## 利用手順
1. GGUF モデルと Sudachi リソースを `include_bytes!` などで静的にバンドルし、`EngineConfig` を構築します。
2. `KazeNhanhEngine::new` でエンジン全体を初期化し、`summarize_document` や `synthesize_summary` などの高レベルメソッドを呼び出します。
3. スレッドから直接推論を行いたい場合は、`KazeNhanhEngine::lock_inference()` で `Arc<Mutex<InferenceEngine>>` を取得し、`synthesize` を呼び出します（内部 API 向け）。

```rust
use kaze_nhanh::{EngineConfig, KazeNhanhEngine};

const MODEL_BYTES: &[u8] = include_bytes!("../resources/llama-q4_0.gguf");
const DICT_BYTES: &[u8] = include_bytes!("../resources/system.dic");
const SETTINGS_BYTES: &[u8] = include_bytes!("../resources/sudachi.json");

fn run_summary(prompt: &str) -> Result<String, kaze_nhanh::KazeNhanhError> {
    let config = EngineConfig::new(MODEL_BYTES, DICT_BYTES, SETTINGS_BYTES);
    let engine = KazeNhanhEngine::new(config)?;

    engine.summarize_document(prompt)
}
```

> **メモ:** `InferenceEngine` は crate 内部専用のため、外部アプリケーションは `KazeNhanhEngine` 経由で利用してください。独自パイプラインで内部 API を使う場合は `kaze_nhanh::inference` モジュールを参照してください。

## 構成とチューニング
- **依存関係**: `candle-core`/`candle-nn`/`candle-transformers` は 0.9 系を使用し、`default-features = false` で CPU バックエンドのみを有効化しています。追加の GPU 機能が必要な場合は、`candle-core` の `cuda`/`metal` 等の feature を将来的に検討します。
- **デバイス強制**: `KazeModel::ensure_cpu_device` により GGUF ロード直後に `Device::Cpu` が検証され、GPU など別デバイスが検出された場合は `ModelLoadError` にフォールバックします。`constructor_forces_cpu_device` テストで CPU スモークチェックを行っています。
- **最大コンテキスト長**: 既定値は 2048 トークンで、プロンプトが超過した場合はスライディングウィンドウで切り詰めます。
- **生成トークン数**: 既定値は 128 トークン。長文出力が必要な場合は将来的に設定値を読み出す API を追加する予定です。
- **サンプリング**: `LogitsProcessor::new` により、固定シード (42)、温度 0.8、Top-p 0.95 を採用しています。外部設定対応は `KZN-ARC-DESIGN-001` のフェーズ 2 で扱います。
- **EOS 検知**: GGUF メタデータ (`tokenizer.ggml.eos_token_id` など) を優先し、既知トークン (`</s>`, `<|eot_id|>`, `<|endoftext|>`) をフォールバックとして探索します。
- **トークナイザー共有**: `Arc<Mutex<Tokenizer>>` でスレッド間共有し、ユニットテスト (`tokenizer_is_shared_across_threads`) でロック取得と並列エンコード動作を確認しています。
- **ストリーミングデコード**: 推論ループ内で生成トークンを段階的に `tokenizer.decode(..., true)` に渡し、UTF-8 の途中断片を許容しながら `streamed_text` を構築します。生成終了時に差分を補完し、最終文字列を `trim()` した上で返却します。
- **エラーステージ**: `stage_error()` ヘルパーでトークナイズ・テンソル初期化・モデル forward・サンプリング・デコードの各段階を特定し、`ModelLoadError`/`ModelInferenceError` に詳細メッセージを伝播します。デコード失敗は `tracing::warn!` で一度だけ通知し、最終デコードで再試行します。

## エラーハンドリング指針
- モデルロード失敗時は `KazeNhanhError::ModelLoadError` に内包した `CandleError` を返し、詳細メッセージをログ出力します。
- 推論エラー (`ModelWeights::forward`、トークナイズ失敗等) は `ModelInferenceError` にマッピングされます。
- ミューテックス獲得失敗など内部的な一時エラーは `CandleError::Msg` に変換し、呼び出し元でリトライ戦略を選択できるようにしています。

## ベンチマーク計画
- **シナリオ**
  - 短文要約プロンプト (<= 256 トークン)
  - 中長文レポート生成 (<= 1024 トークン)
  - Git 差分要約 (連続 diff チャンク)
- **メトリクス**
  - 1 リクエストあたりのレイテンシ (p50/p95)
  - 生成トークン毎秒 (tokens/sec)
  - プロンプトトークン長と生成トークン長のヒストグラム
- **実行コマンド (暫定)**
  - `cargo test synthesize_mock_latency_is_bounded -- --nocapture` (モック推論のヘルスチェック)
  - `cargo bench --bench inference_loop` (将来 Criterion ベンチを追加予定)

### 初期測定結果 (モック環境)
- 測定日: 2025-11-07
- 実行環境: Windows 11 (build 26100), Ryzen 7 7840HS, Rust 1.81.0 `test` プロファイル
- コマンド: `cargo test synthesize_mock_latency_is_bounded -- --nocapture`
- 結果: `mock_synthesize_latency_ns=17800` (約 0.018 ms) — モックモデルのため IO や行列演算は未実行
- 観察: ミューテックス・トークナイズを含めた基本フローは 5ms を大きく下回り、CPU バウンド処理のオーバーヘッドは無視できる水準。実際の GGUF を用いた測定では KV キャッシュの初期化コストが支配的になる見込み。

### 次のアクション
- GGUF 実ファイルを含む評価用バンドルを作成し、Criterion ベンチマーク (`benches/inference.rs`) を追加。
- トークン制限や温度パラメータを `EngineConfig` から外部設定できるよう API を拡張。
- Windows と Linux の双方で同一プロンプトセットを測定し、CPU 固有最適化 (AVX/AVX2) の影響を把握。

