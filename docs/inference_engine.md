> 0.2/P1: この文書はkaze_nhanh_legacyへ分離した旧生成runtimeの検証手順です。rootで旧APIを使うにはlegacy featureが必要。新評価APIはSLMを起動しません。[0.2移行ガイド](migration-0.2.md)。

# 推論エンジンの利用と検証

2026-10-04、Issue #2/P0で本番runtimeとテストfixtureを分離した。

## 本番コンストラクタ

`KazeNhanhEngine::new(config)`はfeatureやcfg(test)に関わらず、本番のCandle CPU backendを使用する。モデルは`general.architecture = llama`のGGUFが対象。

通常のGGUF語彙配列だけではtokenizerを完全に復元できないため、次のどちらかが必要:

- `KazeNhanhEngine::new_with_tokenizer(config, tokenizer_json_bytes)`で完全なtokenizer JSONを渡す。
- GGUFの`tokenizer.json`メタデータに完全なJSONを埋め、従来の`new(config)`を使う。

`EngineConfig`の既存フィールドと`new`は維持する。外部tokenizerを追加する場合の例:

```rust,ignore
use kaze_nhanh::{EngineConfig, KazeNhanhEngine};

const MODEL: &[u8] = include_bytes!("../resources/model.gguf");
const DICTIONARY: &[u8] = include_bytes!("../resources/sudachi/system.dic");
const SETTINGS: &[u8] = include_bytes!("../resources/sudachi/sudachi.json");
const TOKENIZER: &[u8] = include_bytes!("../resources/tokenizer.json");

let config = EngineConfig::new(MODEL, DICTIONARY, SETTINGS);
let engine = KazeNhanhEngine::new_with_tokenizer(config, TOKENIZER)?;
let output = engine.synthesize_summary(vec!["日本語の短い文章です。".into()])?;
```

ロード時にJSONの全語彙IDと`tokenizer.ggml.tokens`を照合し、embedding行数、EOS ID/文字列、architectureを確認する。資産不足・不一致はModelLoadError。PAD/UNKだけのtokenizerへフォールバックしない。語彙照合だけでは正規化・pre-tokenization・chat templateの一致を証明できないため、学習済み資産では別途参照token ID試験が必要。

## 生成の境界

- CPUで固定seed 42、temperature 0.8、top-p 0.95。評価judge用の設定はP3で設計する。
- context上限は2048とGGUFの`llama.context_length`（指定されていれば）の小さい方。
- promptが上限以上ならエラー。先頭や参照文脈を黙って落とさない。
- 最大128生成token、contextの残り予算でも制限する。EOSで停止する。
- 呼出ごとに初期ウェイトをcloneして空のKV cacheから開始する。
- 最終token列を一度decodeする。decode済みprefixが変わる場合の文字欠落を避ける。

## 明示的なモック注入

内部のInferenceBackend traitをInferenceEngineへ注入する。テスト内のfake生成は`InferenceEngine::mock`、外部workflow fixtureは`mock_inference` featureで公開される`test_support::mock_engine`を使う。

`mock_inference`はfixture APIを追加するだけで、通常のコンストラクタをモックへ切り替えない。feature有効時にも、本番の無効GGUF拒否・合成GGUFによる量子化CPU forward試験を実行する。

## 自動試験の範囲

- 合成された小さな1-layer LLaMA GGUFで、実際の量子化CPU forward・KV cache・EOS・decode・context上限を確認。
- 日本語の参照token ID、語彙サイズ/型/ID不一致、欠落JSON、EOS不一致を確認。
- Git/要約workflowは明示fakeで実行。実辞書を使うNLPは別targetでも実行。

合成GGUFは学習済みSLMではない。生成の言語品質や実用モデルのCPU性能を示すものではなく、従来のモックlatency試験も削除した。

## P0の固定学習済みモデル検証

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev/verify-model.ps1 `
  -ModelPath C:\models\model.gguf `
  -TokenizerPath C:\models\tokenizer.json `
  -ReferenceCasesPath C:\models\reference-cases.json `
  -Offline
```

参照caseはJSON配列で、以下の形。expected_idsは、元モデルの公式tokenizer/信頼できる参照実装から`add_special_tokens=true`で取得したものを使う。この例のIDは形式説明用であり実モデルの正解ではない。

```json
[
  {
    "text": "東京都で自然な日本語を解析します。",
    "expected_ids": [1, 100, 200],
    "prompt": "モデル固有のchat templateを適用した日本語prompt"
  }
]
```

runnerは資産SHA256を表示し、参照ID照合、本番ロード、生成が空でないこと、同じpromptの再実行で同じ出力が得られることを確認。出力と各推論の時間をJSON行として標準出力へ返す。モデルやtokenizerの自動取得はしない。

このrunnerは言語品質の自動合否・p95/RSS計測を含まない。P0ではSmolLM2-135M-Instruct Q4_K_M（105,454,432 bytes、Apache-2.0）を互換性確認用に使用する。公式tokenizerとGGUF変換元のrevision・SHA256をresources/models/smollm2.lock.jsonに固定。日本語judgeとしての採用を意味しない。ライフタイムがstaticの旧EngineConfigへ合わせ、CLIの資産bytesはプロセス終了まで保持する。0.2の新評価Sudachiはowned資産を使用するが、このlegacy CLIは0.1のstatic資産契約を維持する。

固定fixtureの取得・検証:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev/setup-model.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev/verify-model.ps1 `
  -ModelPath target/model-smoke/model.gguf `
  -TokenizerPath target/model-smoke/tokenizer.json `
  -ReferenceCasesPath tests/fixtures/smollm2/reference-cases.json -Offline
```

一度取得すればsetup-model.ps1にも`-Offline`を指定できる。欠落・hash不一致は停止する。資産はignored target内へ保存し、通常の開発セットアップではモデル取得を必須にしない。CIのTrained GGUF CPU & Official Tokenizer Smokeジョブが同じfixtureを検証し、trained-model-smoke成果物へログを保存する。

参照IDはRustとは別バージョンの公式Python実装で生成済み。再生成時だけPython/uvが必要:

```powershell
uv venv --python 3.12 target/model-reference-venv
uv pip install --python target/model-reference-venv/Scripts/python.exe -r scripts/dev/model-reference-requirements.txt
target/model-reference-venv/Scripts/python.exe scripts/dev/generate-model-references.py `
  target/model-smoke target/model-smoke/regenerated-cases.json
```

再生成JSONはtests/fixtures/smollm2/reference-cases.jsonと一致すること。通常のRust検証にPython/PyTorchは不要。[参照fixtureの出典](../tests/fixtures/smollm2/README.md)も参照。
