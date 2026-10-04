# 0.1 → 0.2移行ガイド

2026-10-04、P1でcrateを分離した。0.2は開発版。P2でPrimaryRulesとDomainProfileを標準経路へ追加した。[一次検出の範囲](primary-detection.md)を参照。

## 新しい評価API

```toml
[dependencies]
kaze_nhanh = { path = "/path/to/KazeNhanh" }
```

defaultはSudachiのみ。モデル、Git、Markdown、Candle、生成tokenizerは通常依存に含まない。

```rust,ignore
use kaze_nhanh::{japanese_engine, EvaluationConfig, SudachiConfig, SudachiMode, TextInput};

let assets = SudachiConfig::from_paths("system.dic", "sudachi.json", SudachiMode::C)?;
let engine = japanese_engine(assets, EvaluationConfig::default())?;
let report = engine.evaluate(TextInput::new("東京都で日本語を解析します。"))?;
```

dictionary/settingsはowned bytesとしてengineに保持される。static化やBox::leakは不要。辞書・設定SHA256とModeをprovenanceへ記録し、助詞・助動詞を含む全形態素を原文位置で解析する。

P2の標準経路はprofileの制約と少数ルールを評価する。validity/naturalnessはheuristic、semantic_consistencyはnull。必要な二次判定はdisabled/保留で、SLM呼出0。形態素解析成功だけを「妥当」へ置換しない。独自PrimaryDetectorも注入できるが、品質受入は利用者が検証する。Report schemaはP3制御契約のv3へ更新。

```powershell
cargo run --locked --example evaluate -- "東京都で自然な日本語を解析します。"
```

## 最小core / 別backend

```toml
kaze_nhanh = { path = "/path/to/KazeNhanh", default-features = false }
```

`EvaluationEngine::new(Arc<dyn MorphAnalyzer>, EvaluationConfig)`で別backendを明示注入する。coreの通常依存はserde/serde_json/thiserrorだけ。評価ErrorやMorphemeにSudachi/Candle/Gitの固有型は含めない。

## 旧Git・要約API

```toml
kaze_nhanh = { path = "/path/to/KazeNhanh", default-features = false, features = ["legacy"] }
```

旧KazeNhanhEngine、EngineConfig、foundation、GitNativeRAG出力等はlegacy featureで同じroot pathへ再exportされる。旧APIは従来どおりモデルとstatic辞書を必要とし、要約の意味を維持する。新evaluate APIへ意味を変更しない。

またはworkspace内の`kaze_nhanh_legacy` crateを直接依存に指定する。旧コードのimport先をkaze_nhanh_legacyへ変更し、従来のEngineConfigを使う。旧runtimeの資産検証は[推論エンジン](inference_engine.md)を参照。

## Featureと検証

| 構成 | 評価core | 新Sudachi | 旧Git/Markdown/Candle | 用途 |
| --- | --- | --- | --- | --- |
| default | 有効 | 有効 | 無効 | 標準のモデル不要評価 |
| no-default-features | 有効 | 無効 | 無効 | 別backend/最小build |
| no-default + legacy | 有効 | 無効 | 有効 | 旧APIの互換利用 |
| mock_inference | 有効 | defaultに従う | 有効 | 明示fake workflow試験 |

mock_inferenceはlegacyを有効にし、通常コンストラクタをfakeへ切り替えない。P3のSecondaryJudge/Factory/Workerはbackend非依存契約として利用できる。実SLM adapterは未実装で、legacy推論featureを新評価エンジンへ接続しない。

`verify.ps1 -Offline`はcore/default/minimalの依存禁止、実Sudachi、Modeと原文span、並行評価、旧本番runtime、fake workflow、全workspace試験を確認する。辞書はsetup-dev.ps1で事前取得する。

P3制御契約: optional SecondaryWorkerを明示接続すると選択的judgeを実行できる。標準CLIはworker未接続。reference不足はcontext_missing/呼出0、不正出力・予算・timeoutは保留。[接続・制約・実モデル残作業](secondary-judging.md)。
