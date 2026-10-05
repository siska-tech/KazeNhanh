# KazeNhanh

![KazeNhanh Mascot](docs/img/KazeNhanh.png)

KazeNhanhは、OCR/ASRの機械認識テキストについて、形態素・語彙・文字列統計・言語モデル・認識器のconfidenceと候補情報を統合し、異常・不確実性・誤認識リスクを評価するローカル基盤を目指します。detection first / CPU-first / local-firstを維持し、naturalnessは補助指標とします。

**recognition risk中心のR0/R1 APIを追加しました。** 無警告はundetermined、一次異常根拠はreview、riskは未推定/nullとして返します。型付きconfidence/N-best、OCR/ASR位置adapter、欠測とprofileをR1で追加。R2で文字/語n-gramの統計evidenceとローカル資産を先行追加。検出/fusion、risk推定・校正は未実装です。[統計APIと手順](docs/recognition-statistics.md)。[Source evidence](docs/recognition-source-evidence.md)。[新APIとCLI](docs/recognition-api.md)。 現在の0.2開発版はcore/Sudachi/legacy分離、限定的な一次screening、任意workerの遅延load・有界queue・予算制御を実装しています。現APIのacceptableは認識正解・低リスクを保証せず、旧APIの無警告acceptableを新APIの低リスクへ転用しません。新設計では証拠不足を判断不能として扱い、Qwen Naturalness Judgeは実験比較用へ位置付けます。[再設計案と移行順](docs/KZN-REDESIGN-PLAN-002.md) / [現screeningの範囲](docs/primary-detection.md)。

## 利用開始

OCR/ASR向けのR0実行例: `cargo run --locked --offline --example recognize -- "節約する" ocr image-001 line-02`。証拠不足のためrisk=null / undeterminedとなります。以下のevaluate例は従来のscreening APIです。

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

- 2026-10-04: R2統計evidenceを先行実装。文字/語n-gram、domain/辞書版照合、clean corpus builder、hash検証と1回解析を追加。独立実OCR/ASR集合、異常rule/fusion・品質比較が残り、R2はprogress。
- 2026-10-04: R1 typed source evidenceを実装。confidence尺度/粒度/欠測、N-best/UTF-8 alignment、OCR bbox/ASR時刻adapter・profile、モデル不要runnerを追加。report schemaはkzn.recognition.v2。提供OCR15件の保持・全件保留、合成OCR/ASR契約と回帰を検証。次はR2。
- 2026-10-04: R0契約・保留APIを実装。別schema、evidence欠測、原文保持、risk=null、review/undetermined、モデル不要CLIを追加。提供OCR15件の保留・confidence保持を検証。R1が次の段階。
- 2026-10-04: OCR実測を受け、recognition risk中心の[再設計案002](docs/KZN-REDESIGN-PLAN-002.md)を作成。R0契約/保留 → R1 OCR/ASR evidence → R2軽量baseline → R3選択LM/判別器 → R4校正/受入を次の実装順とします。以下のP0〜P3は従来設計での実装履歴です。

- 2026-10-04: P2一次検出MVPを実装。profile・全形態素features・説明可能rules、4用途共通API、JSON runnerと34件baselineを追加。正常fixture19件で誤警報0、意味保留4件、SLM呼出0。実データの品質保証はP3/P4で評価。

- 2026-10-04: P1完了。ユーザー承認後にcore/Sudachi/legacyをworkspaceへ分離し、0.2へ切替。モデル不要の標準facade、最小feature、legacy互換と移行ガイドを整備。
- 2026-10-04: P0完了。実辞書、明示fake注入、学習済みGGUF CPU生成、公式tokenizer参照ID、CI成果物を検証。
- P3進行中: 選択的worker・遅延load・有界queue・共有呼出予算・deadline・厳格出力/保留の契約を実装。優先用途はOCR/ASR。実験用Qwen自然さadapterを追加し、日本語品質/意味adapter/CPU SLO検証は残作業。[二次判定の契約](docs/secondary-judging.md)。

[移行タスク](tasks/task-redesign-002-text-evaluation-migration.md) / [ロードマップ](tasks/ROADMAP.md) / [Issue #2](https://github.com/siska-tech/KazeNhanh/issues/2) / [Draft PR #3](https://github.com/siska-tech/KazeNhanh/pull/3)

## 仕様・設計

- [recognition risk再設計案002（今後の方針）](docs/KZN-REDESIGN-PLAN-002.md)
- [初回監査・計画001（履歴）](docs/KZN-REDESIGN-PLAN-001.md)
- [API契約002](docs/KZN-API-SPEC-002.md)、[構成002](docs/KZN-ARC-DESIGN-002.md)、[要件002](docs/KZN-REQ-SPEC-002.md)
- 001仕様と旧タスクはGit/要約機能の履歴として保持します。

## English

KazeNhanh targets local, detection-first recognition risk assessment for OCR and ASR. The proposed design combines text features with recognizer evidence and treats naturalness as an auxiliary signal. The R0 recognition API returns review for primary findings and undetermined otherwise, with risk unestimated. R1 adds typed confidence, N-best alignment and source adapters; R2 adds opt-in corpus-relative character/word frequencies with domain and analyzer identity checks; detection/fusion and risk estimation remain pending. The recognition report schema is kzn.recognition.v2. The separate 0.2 evaluation API provides limited text screening, and acceptable does not establish recognition correctness. See the [redesign proposal](docs/KZN-REDESIGN-PLAN-002.md). P1 separates the backend-independent core, owned Sudachi adapter and optional legacy Git/Markdown/generation APIs. The default build needs no language model; `default-features = false` exposes only the core facade.

P2 includes explainable primary screening and profiles. Semantic consistency remains unassessed; unresolved candidates are explicitly held with the secondary judge disabled. Heuristic scores are not calibrated probabilities. P3 now provides an opt-in bounded lazy worker and strict secondary protocol; an experimental Qwen naturalness adapter is available, while Japanese quality/semantic adapter/CPU SLO acceptance remain pending. P4 adds calibration and quality acceptance. Legacy APIs require the `legacy` feature or direct use of kaze_nhanh_legacy. See the [migration guide](docs/migration-0.2.md).

2026-10-04: 実験用Qwen CPU自然さadapterを独立optional crate/qwen featureへ追加。固定GGUF/tokenizer、独立参照、選択的CPU smokeを実装。P3は品質/意味adapter/CPU SLO未受入のためprogress。[手順と限界](docs/qwen-judge.md)。

実OCRサンプル: PP-OCRv6 mediumの3画像/15件を原文・confidence付きで保存。最初の5件と追加2箇所は転記確認済み、残り8件は未確認。runner既定はモデル不要（SLM呼出0）、Qwen比較は明示opt-in。現gateの検出漏れと実験用Qwenの未受入品質を記録。[初回実測](docs/ocr-samples-user-001.md) / [追加データと観察](docs/ocr-samples-user-002-003.md)。

2026-10-04: [small/core/full辞書比較](docs/sudachi-dictionary-matrix.md)を追加。同版・Mode CでOCR15件と人工hard-cleanを観察し、統計assetを辞書別に生成。分割差はあるがOCR判定は全件保留のまま。実OCR/ASR・fusion/CPU品質の受入は継続。

2026-10-05: [R2候補review baseline](docs/recognition-candidate-review.md)を追加。raw候補不一致を明示opt-inでreviewへ送り、rank/spanを保存。人工12例を3辞書で比較。候補の正解性・誤り確率は未推定で、R2実品質受入は継続。

2026-10-05: [R2オフライン品質集計](docs/recognition-quality.md)を追加。確認済み転記と未確認を分け、review・保留・low_riskの件数と分母を保持。small/core/fullのローカルmatrixに統合。実データでの受入・統合判定は継続。

2026-10-05: [R2評価データのsplit監査](docs/recognition-datasets.md)を追加。document/origin/話者/session・同一転記のsplit跨ぎと未確認testを拒否。公開ASR説明例1対は固定取得し、実測品質データへは昇格させない。

2026-10-05: [R2軽量統計review](docs/recognition-sparse-review.md)を追加。OOVと未観測文字/語bigramの共起をopt-inでreviewへ回し、候補reviewと併用可能。誤り確率・品質受入とは区別する。追加サンプルは必要時にユーザーへ依頼する。

2026-10-05: [R2 POS bigram統計](docs/recognition-pos-statistics.md)を追加。全品詞vectorの隣接頻度・未観測率と欠測を分離し、v2統計assetを明示生成。旧v1資産と既定の判定は維持。

文字種・遷移の原文観測を追加（2026-10-05）。判定には未使用、旧レポート読み込みを維持。[仕様](docs/recognition-string-features.md)。

POS・文字種のoffline条件比較を追加（2026-10-05）。単純条件の本番採用は見送り、既存判定を維持。[比較結果](docs/recognition-feature-ablation.md)。

source固有confidenceの明示ruleをadapterへ追加（2026-10-05）。尺度・認識器等が一致する観測だけを比較し、適用不能を保持。[契約](docs/recognition-confidence-review.md)。engine統合・推奨閾値は未実装。

ユーザー提供OCR合成画像データ1,000件を登録（2026-10-06）。固定hash・split監査・goldを分離したローカル取り込みを追加。[利用方針](docs/ocr-synth-1k.md)。

OCR 1kのtrain500件によるPOS付き統計assetと、small/core/fullのdevelopment100件比較runnerを追加（2026-10-06）。null confidence対応・小数のJSON往復修正を含む。[評価手順](docs/ocr-synth-1k.md)。

source confidenceのengine接続・統合レポートを追加（2026-10-06）。既存reviewと確認要求をOR統合し、高confidenceでもreviewを取り消さない。[契約](docs/recognition-confidence-review.md)。
