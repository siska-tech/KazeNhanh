# 実験用Qwen CPU自然さadapter

> 2026-10-04方針変更: [recognition risk再設計案002](KZN-REDESIGN-PLAN-002.md)では、本judgeを主経路の採用対象から外し、実験比較用に保持する。以下は既存opt-in adapterの実装・検証手順。risk判定や本文LM surprisalの実装ではない。

2026-10-04 / P3進行中。OCR/ASRの自然さ専用。Qwen2.5-0.5B-Instruct Q4_K_Mをoptional `kaze_nhanh_qwen` crateへ実装し、rootでは`qwen` featureで明示有効化する。core/default/minimalにはCandle/tokenizerを追加しない。Qwen crateはGit/Markdown/legacy/Sudachiへ依存しない。

## セットアップと利用

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev/setup-judge.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev/verify-judge.ps1 -Offline
```

セットアップだけがネットワークを使う。固定revision・SHA256のGGUF（491,400,032 bytes）/公式tokenizer/config/chat template/LICENSEをtarget/qwen-judgeへ配置する。offlineは欠落・不一致を拒否する。runtimeはローカル資産を検証し、download/fallbackを行わない。setup-dev.ps1はjudgeを取得しない。

```rust,ignore
let factory = Arc::new(QwenNaturalnessFactory::new(QwenJudgeConfig::local("target/qwen-judge"))?);
let worker = Arc::new(SecondaryWorker::new(factory, SecondaryPolicy::default())?);
let engine = japanese_engine(sudachi, config)?.with_secondary_worker(worker);
```

QwenJudgeConfig::localの既定は1024 context tokens、label mass下限0.01。これらは実験用保護値で品質受入/SLOではない。自然さ軸だけを要求するOCR/ASR profileへ接続する。semantic_consistencyを要求された場合はinvalid_outputで保留し、自然さを意味/事実性に流用しない。

## 判定方式と限界

公式chat templateのsystem/user/assistant境界で、日本語入力をJSON dataとして渡す。JSON内の`<`をescapeし、本文のchat delimiterを新しいroleとしてtokenizeしない。原文はreportへ保持する。この構造化だけで自然言語のprompt injectionを完全防御したとは主張しない。

自由生成ではなく1回のCPU forwardで「1=自然」「0=不自然」のlogitsを取り出す。2ラベル内のsoftmax比をnaturalness model scoreとし、全語彙に対する2ラベルmassもevidenceへ返す。低mass/非finite/非対応軸は保留。scoreは未校正で、自然な文である確率ではない。threshold/scoreが良いかは実データで検証する必要がある。model scoreが高くても一次warningは削除しない。

GGUF architecture=qwen2、全使用tokenの文字列/ID、embedding rows、公式EOS/config、assets hashを検証する。Qwenのpaddingされたembedding rowsはtokenizerへ捏造追加せず、使用token全件を照合する。固定資産以外のモデル互換性を保証しない。

prompt＋decision 1tokenがcontextを超えればbudget_exceededで保留し、本文をtruncateしない。各forwardのindex_pos=0でCandleのKVをresetし、異なるpromptの後でも再実行scoreが一致することをsmokeで確認する。loadの読取りchunkとforwardの前後にdeadlineを確認するが、1回のforward中の強制中断はできない。workerがtimeoutした場合でもin-flight処理が戻るまではbusyとなる。[制御契約](secondary-judging.md)。

## 独立照合とCPU smoke

scripts/dev/generate-judge-references.pyは固定Python環境（transformers 4.46.3/tokenizers 0.20.3）から公式chat templateを適用し、4件の日本語/混在文字/delimiter参照を生成する。Rust tokenizers 0.19.1がraw textとpromptの全IDを照合する。参照はtests/fixtures/qwen/reference-cases.jsonで管理。

judge_smokeは正常文の呼出0、3種の異常候補、異なるprompt後のrepeat、context超過保留をJSONへ記録する。独立CIでrelease CPUを実行しqwen-judge-smoke成果物を保存する。これは少数の動作・再現性smokeで、実OCR/ASRのprecision/recall、RSS SLOや採用判定ではない。

[公式model card](https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct)、[公式GGUF](https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct-GGUF)。日本語対応は候補選定根拠であり、日本語judge品質の証明ではない。

P3残作業: 実OCR/ASRの正常/誤認識データ、prompt injection/曖昧例、意味adapter、協調deadlineの運用制約、対象CPU/RAM/thread数とSLOの事前定義、品質・性能受入。P4ではgateの見逃しも含む比較とcalibrationを行う。
ローカルrelease smokeの品質観察: 異常候補3件はいずれもnaturalnessが0.5以上（反復0.99986、文字化け0.82134、括弧欠落0.99728）。このprompt/modelの判定品質は未受入。thresholdを後付け調整して採用済みとはしない。一次warningは保持。参考実行時間14〜17秒/候補で、SLO計測ではない。workerのcallsはjudge試行数で、context/非対応軸の事前拒否も数える。actual_forwardsはCPU forwardのみを別に記録する。
