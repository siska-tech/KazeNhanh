---
status: completed
priority: high
assignee: Backend
parent: task-inference-001-inference-engine
start_date: 2025-11-07
end_date: 2025-11-07
tags: ["inference", "analysis"]
depends_on: []
---

# サブタスク概要
CandleおよびGGUFサポートの最新状況を調査し、必要なcrateバージョンやfeatureフラグ、既知の制約をまとめる。

## 完了条件
- 依存crateのバージョン決定とCargo設定案がまとまっている。
- CPUバックエンド利用時の注意点や性能要件が整理されている。
- モデルファイルの配置・読み込み方針が記載されている。

## 調査メモ
- `candle_core::quantized::gguf_file::Content` を用いてGGUFのメタデータおよびテンソル情報を抽出できる。
- 量子化テンソルのメモリ展開には `candle_transformers::quantized_var_builder::VarBuilder::from_gguf_buffer` が利用可能で、ファイルではなくバイト列から直接読み込める。
- LLaMA系GGUFは `candle_transformers::models::quantized_llama::ModelWeights::from_gguf` でロードでき、`Device::Cpu` を指定するだけでCPU実行を強制できる。
- GGUFメタデータには `tokenizer.json` エントリが含まれる想定のため、`tokenizers::Tokenizer::from_bytes` で復元し `Arc<Mutex<Tokenizer>>` に包む構成を採用する。
- テキスト生成は `candle_transformers::generation::LogitsProcessor` のトップP/温度サンプリングで制御でき、シード固定により再現性が確保できる。

## 進行状況
- 2025-11-07: Candle 0.9 系（`candle-core`/`candle-nn`/`candle-transformers`）を default-features 無効で採用する Cargo 設定を確定し、`cargo check --release` にて CPU 専用構成でのコンパイルを確認。GGUF モデルは `include_bytes!` 等で静的埋め込みする方針とし、`docs/inference_engine.md` にデバイス強制・トークナイザー復元・ベンチマークの注意点を整理済み。

