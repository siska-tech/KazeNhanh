# KazeNhanh

![KazeNhanh Mascot](docs/img/KazeNhanh.png)

形態素解析と選択的SLMを組み合わせる、detection first / CPU-first / local-firstのテキスト妥当性評価基盤を開発しています。0.2開発版では評価core・Sudachi backend・旧Git/要約機能を分離しました。

**現在はP1まで完了。標準の検出器は未実装です。** 形態素解析と原文/span保持が動き、評価結果はundetermined・三軸score=nullを返します。一次検出MVPはP2、選択的SLMはP3、校正・品質受入はP4で追加します。

## 利用開始

新PCの準備と固定辞書の取得は[セットアップ手順](docs/development-setup.md)を参照してください。

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev/setup-dev.ps1
cargo run --locked --example evaluate -- "東京都で自然な日本語を解析します。"
```

GGUFモデルの配置は不要です。上のexampleはSudachi解析を行い、未評価reportをJSONで返します。通常の評価実行でdownloadやcloud fallbackは行いません。

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

入力sourceはOCR・ASR・LLM・Form・PlainText等。画像/音声処理は呼出側が行い、coreはテキストと任意の参照文脈・annotationsを受理します。原文UTF-8 byte spanを保持し、未評価・文脈不足・backend失敗を正常へ変換しません。訂正文は返しません。

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

- 2026-10-04: P1完了。ユーザー承認後にcore/Sudachi/legacyをworkspaceへ分離し、0.2へ切替。モデル不要の標準facade、最小feature、legacy互換と移行ガイドを整備。
- 2026-10-04: P0完了。実辞書、明示fake注入、学習済みGGUF CPU生成、公式tokenizer参照ID、CI成果物を検証。
- 次工程: P2一次検出MVP。正常語・短文・固有名詞も含むbaselineと説明可能rulesを追加。

[移行タスク](tasks/task-redesign-002-text-evaluation-migration.md) / [ロードマップ](tasks/ROADMAP.md) / [Issue #2](https://github.com/siska-tech/KazeNhanh/issues/2) / [Draft PR #3](https://github.com/siska-tech/KazeNhanh/pull/3)

## 仕様・設計

- [再設計監査・計画](docs/KZN-REDESIGN-PLAN-001.md)
- [API契約002](docs/KZN-API-SPEC-002.md)、[構成002](docs/KZN-ARC-DESIGN-002.md)、[要件002](docs/KZN-REQ-SPEC-002.md)
- 001仕様と旧タスクはGit/要約機能の履歴として保持します。

## English

KazeNhanh 0.2 is a local, detection-first text evaluation foundation. P1 separates the backend-independent core, owned Sudachi adapter and optional legacy Git/Markdown/generation APIs. The default build needs no language model; `default-features = false` exposes only the core facade.

The standard detector is not implemented yet: reports explicitly return undetermined verdicts and null scores. P2 adds primary detection, P3 selective SLM judging, and P4 calibration and quality acceptance. Legacy APIs require the `legacy` feature or direct use of kaze_nhanh_legacy. See the [migration guide](docs/migration-0.2.md).
