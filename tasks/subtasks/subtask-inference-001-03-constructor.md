---
status: completed
priority: high
assignee: Backend
parent: task-inference-001-inference-engine
start_date: 2025-11-07
end_date: 2025-11-07
tags: ["inference", "ai"]
depends_on: ["subtask-inference-001-02-tokenizer"]
---

# サブタスク概要
`InferenceEngine::new`を実装し、GGUFファイルからモデルを読み込み、CPUデバイスで推論可能な状態に構築する。

## 完了条件
- `VarBuilder::from_gguf_decompressed`を用いたモデルロードが成功する。
- `Device::Cpu`での実行が保証され、GPU機能が無効化されている。
- ロード処理に関するユニットテストまたはスモークテストが存在する。

## 進捗メモ
- GGUF読み込みは`ModelWeights::from_gguf`と`Device::Cpu`を利用し、CPU専用でロード。
- ロード結果は`KazeModel`にキャッシュし、`Arc<Mutex<Tokenizer>>`と共に`InferenceEngine`へ移譲。
- 空バイト列に対する失敗パスを単体テストで検証済み。

## 進行状況
- 2025-11-07: `KazeModel::ensure_cpu_device` と `InferenceEngine::is_cpu_device` を追加し、GGUFロード直後にCPUデバイス強制を検証。`constructor_forces_cpu_device` テストでCPU専用パスを確認し、ロード失敗時には `ModelLoadError` にマッピングされるように調整。

