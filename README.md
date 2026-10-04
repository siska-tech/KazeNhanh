# KazeNhanh

![KazeNhanh Mascot](docs/img/KazeNhanh.png)

形態素解析と選択的SLMを組み合わせる、detection first / CPU-first / local-firstのテキスト妥当性評価基盤を開発しています。0.2開発版では評価core・Sudachi backend・旧Git/要約機能を分離しました。

**P2一次検出MVPは完了、P3の二次制御契約を実装中です。** profileの入力制約と文字化け候補・括弧・形態素/文字の反復を検出します。意味整合性は未評価で、必要な二次判定を保留として返します。任意workerの遅延load・有界queue・予算・保留を実装済みです。実SLM adapterはP3、校正・品質受入はP4で追加します。[検出範囲とbaseline](docs/primary-detection.md)。

## 利用開始

新PCの準備と固定辞書の取得は[セットアップ手順](docs/development-setup.md)を参照してください。

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev/setup-dev.ps1
cargo run --locked --example evaluate -- "東京都で自然な日本語を解析します。"
```

GGUFモデルの配置は不要です。上のexampleはSudachi解析と一次screeningを行い、score・issue・原文span・未評価範囲をJSONで返します。通常の評価実行でdownloadやcloud fallbackは行いません。

ローカルアプリケーションの依存例:

```toml
[dependencies]
kaze_nhanh = { path = "../KazeNhanh" }
```

```rust,ignore
use kaze_nhanh::{japanese_engine, EvaluationConfig, SudachiConfig, SudachiMode, TextInput};

let assets = SudachiConfig::from_paths("system.dic", "sudachi.json", SudachiMode::C)?;
let engine = japanese_engine(assets, EvaluationConfig::default())?;
let report = engine.evaluate(TextInput::new("短い入力です。"))?;
```

入力sourceはOCR・ASR・LLM・Form・PlainText等。画像/音声処理は呼出側が行い、coreはテキストと任意の参照文脈・annotationsを受理します。原文UTF-8 byte spanを保持し、未評価・文脈不足・backend失敗を正常へ変換しません。acceptableもprofileと実装ルールの範囲に限定し、意味の正しさを保証しません。訂正文は返しません。

## 構成と互換性

| crate / feature | 責務 |
| --- | --- |
| kaze_nhanh_core | Input/Report/Error、三軸score、原文span、backend trait。serde/serde_json/thiserrorのみ |
| kaze_nhanh_sudachi / default | owned辞書、Mode A/B/C、全形態素、辞書/設定hash。モデル不要 |
| kaze_nhanh / no-default-features | coreだけのfacade。MorphAnalyzerを明示注入 |
| kaze_nhanh_legacy / legacy | 旧Git・Markdown・RAG・要約・Candle runtime |
| mock_inference | legacy workflowへの明示fake注入。通常コンストラクタは本番runtime |

旧KazeNhanhEngine/EngineConfig等を使う場合は`features = ["legacy"]`を指定するか、kaze_nhanh_legacyへ直接依存してください。defaultのAPI availability変更を0.2の変更として扱います。[0.2移行ガイド](docs/migration-0.2.md)と[旧推論資産の検証](docs/inference_engine.md)を参照。packageの公開・PRのmergeは未実施です。

## 検証

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev/verify.ps1 -Offline
```

core/default/minimalの依存境界、原文/span/未評価/schema契約、モデル不要の実Sudachi、Mode・並行評価、旧本番runtime、結合/並行性workflowを検証します。CIはWindows/Linux QA、TSan、学習済みGGUF CPU smoke、Criterion/Soakを実行します。Criterion/Soakは旧モックworkflowの測定であり、新評価器の品質・SLM性能を示しません。

## 開発進捗

- 2026-10-04: P2一次検出MVPを実装。profile・全形態素features・説明可能rules、4用途共通API、JSON runnerと34件baselineを追加。正常fixture19件で誤警報0、意味保留4件、SLM呼出0。実データの品質保証はP3/P4で評価。

- 2026-10-04: P1完了。ユーザー承認後にcore/Sudachi/legacyをworkspaceへ分離し、0.2へ切替。モデル不要の標準facade、最小feature、legacy互換と移行ガイドを整備。
- 2026-10-04: P0完了。実辞書、明示fake注入、学習済みGGUF CPU生成、公式tokenizer参照ID、CI成果物を検証。
- P3進行中: 選択的worker・遅延load・有界queue・共有呼出予算・deadline・厳格出力/保留の契約を実装。優先用途はOCR/ASR。実験用Qwen自然さadapterを追加し、日本語品質/意味adapter/CPU SLO検証は残作業。[二次判定の契約](docs/secondary-judging.md)。

[移行タスク](tasks/task-redesign-002-text-evaluation-migration.md) / [ロードマップ](tasks/ROADMAP.md) / [Issue #2](https://github.com/siska-tech/KazeNhanh/issues/2) / [Draft PR #3](https://github.com/siska-tech/KazeNhanh/pull/3)

## 仕様・設計

- [再設計監査・計画](docs/KZN-REDESIGN-PLAN-001.md)
- [API契約002](docs/KZN-API-SPEC-002.md)、[構成002](docs/KZN-ARC-DESIGN-002.md)、[要件002](docs/KZN-REQ-SPEC-002.md)
- 001仕様と旧タスクはGit/要約機能の履歴として保持します。

## English

KazeNhanh 0.2 is a local, detection-first text evaluation foundation. P1 separates the backend-independent core, owned Sudachi adapter and optional legacy Git/Markdown/generation APIs. The default build needs no language model; `default-features = false` exposes only the core facade.

P2 includes explainable primary screening and profiles. Semantic consistency remains unassessed; unresolved candidates are explicitly held with the secondary judge disabled. Heuristic scores are not calibrated probabilities. P3 now provides an opt-in bounded lazy worker and strict secondary protocol; an experimental Qwen naturalness adapter is available, while Japanese quality/semantic adapter/CPU SLO acceptance remain pending. P4 adds calibration and quality acceptance. Legacy APIs require the `legacy` feature or direct use of kaze_nhanh_legacy. See the [migration guide](docs/migration-0.2.md).

2026-10-04: 実験用Qwen CPU自然さadapterを独立optional crate/qwen featureへ追加。固定GGUF/tokenizer、独立参照、選択的CPU smokeを実装。P3は品質/意味adapter/CPU SLO未受入のためprogress。[手順と限界](docs/qwen-judge.md)。

実OCRサンプル: ユーザー提供PP-OCRv6 mediumの5件を確認済み画像原文/confidenceと保存。現gate/実験用Qwenの見逃しを含めて記録。[データと実測](docs/ocr-samples-user-001.md)。
