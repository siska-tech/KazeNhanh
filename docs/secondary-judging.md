# P3選択的二次判定の制御契約

2026-10-04。優先用途はユーザー指定の日本語OCR/ASRの不自然さ検出。今回実装したのはbackend非依存の実行制御と契約。実験用Qwen自然さbackendを追加したが、意味adapter・日本語品質・CPU SLOは未受入であり、P3全体を完了にしない。

## 接続

SecondaryJudgeFactoryがartifact identityとload(deadline)を提供し、SecondaryJudgeがjudge(SecondaryRequest)を実装する。coreはモデルを知らず、Candle/Git依存を追加しない。SecondaryWorkerをArcで共有して、EvaluationEngine::with_secondary_workerで明示接続する。標準japanese_engine/CLIにはworkerを接続しないので従来どおりモデル不要・呼出0。

```rust,ignore
let worker = Arc::new(SecondaryWorker::new(factory, SecondaryPolicy::default())?);
let engine = japanese_engine(sudachi, config)?.with_secondary_worker(worker.clone());
let report = engine.evaluate(TextInput::new("今日は晴れですですです。"))?;
```

factoryはartifact宣言時に重いモデルloadを行わない。workerの構築時にはスレッドとキューだけを作成する。候補受理後に1回だけloadする。成功したモデルを再利用し、load失敗はcacheして再試行stormを防ぐ。変更した資産で再試行する場合は新workerを明示構築する。

## Gate・予算・保留

- detector未設定、正常一次screening、確定constraint違反は二次呼出0。モデルloadも0。
- suspicious/未評価の必須軸/参照評価候補に限って二次へ進む。reference scopeで参照欠落/空白ならcontext_missing・insufficient_context、load/呼出0。文脈不足をモデルで補完しない。
- 呼出予算はworker共有の累積max_calls（失敗したjudgeも消費）。入力ごとやbatchごとにresetしない。予算の単位・更新はアプリ側がworkerのlifetimeで明示する。
- text+referenceのbyte上限、出力token予算、1つのworker＋有界待機キュー、queue待機を含むdeadlineを設ける。既定は1000calls/16KiB/256生成tokens/30秒/待機1件。これは初期の保護設定であり、製品SLOではない。
- byte/call予算超過・queue飽和はbudget_exceeded、deadline超過はtimeout、load/judge失敗・panicはbackend_error、契約不適合はinvalid_output。いずれもundeterminedへ保留し、原文と一次issueを残す。
- timeout後の待機requestはcancelし、後で無用な推論を開始しない。既にjudge開始済ならslm_calls=1、load/queue待機中に終了した場合は0。late outputで返却済reportを更新しない。

同期backendの実行中のforwardやファイル読取りをcoreが強制停止することはできない。backendはdeadlineをload/forward間で確認し、token/context上限を自ら検証する必要がある。非協調backendが停止しなくても追加workerを増殖させず、そのworkerはbusyのまま。timeoutやDropで強制killしたとの保証はしない。workerはchannel切断後、実行中処理が戻れば終了する。

## 出力とschema

SecondaryEvaluationは要求されたnaturalness/semantic_consistencyのUnitScoreとsecondary issueだけを返す。validity・verdict・correctionは出力フィールドに含めない。JSON parserは未知field、不正score、64KiB超の出力を拒否する。要求軸の欠落/余分な軸、不正UTF-8 span/原文範囲、primary stage、空code/説明、過大issue/根拠を拒否する。実backendで生成JSONから復元する際はこのparserを使用する。

model scoreはmethod=model、confidence/calibration_idはnull。暫定閾値0.5未満なら全原文spanのwarningを追加し、score/thresholdをevidenceに保存。校正済確率や日本語精度の保証ではない。一次warningは二次の高scoreで消去せずsuspiciousに残す。将来の根拠付きresolveは別契約として設計する。

二次失敗/skipで対象軸はfailed/nullへ変更し、coverageを未評価へ戻す。成功時のみ要求軸の全文coverageをmodel評価済とする。未要求の意味軸を合格扱いしない。validityの明示制約scoreは維持する。

Report schemaはkzn.evaluation.v3。context_missing/invalid_outputのrouting状態とCompleted時のcall整合性を追加したため、旧v2をvalidateで拒否する。profile/featuresの構造は継続。P2 fixtureの期待判定は変えず、新schemaで再実行する。

## 検証と未完了事項

明示fakeで正常/invalid/参照不足のload0、共有lazy load、要求軸、原文保持、JSON/score/span拒否、failed load cache、panic保留、呼出/byte予算、cold timeout、queue飽和、期限切れrequestの非実行を検証。実Sudachi＋fake judgeでASR助動詞の反復spanをそのまま渡すことも検証する。これらはモデルの日本語判定品質を示さない。CIのThreadSanitizerへcore secondary契約試験を追加する。

最初の実モデル候補は[公式Qwen2.5-0.5B-Instruct](https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct)と[公式GGUF](https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct-GGUF)。公式cardは日本語を含む多言語とJSON出力対応を記載している。既存Candle 0.9.1にはquantized_qwen2実装があり、legacy backendはllama限定なので流用接続せず独立adapterを検討する。これは採用・精度確認ではなく、小型CPU候補という実装上の判断。

残作業:

- 固定revision/hash/license付きmodel+tokenizer+chat template bundleを別セットアップで取得し、offline再利用・不一致拒否・独立token IDを照合。
- 実験用自然さadapterの実データ検証、意味adapter、request間KV・deadline協調の運用制約を受入確認。
- 日本語OCR/ASRの正常文・固有名詞・filler・短文・実認識誤りで実モデル試験。prompt injection/不正JSONを保留する。
- 対象CPU/RAM/thread数・入力長を固定し、cold/warm latency/RSSと呼出率を測定。CPU SLOと品質受入を事前に定義する。
P3実験用adapter: optional qwen feature/crateでCPUの自然さpaired-label判定を追加。固定資産セットアップと独立token ID/CPU smokeは[Qwen手順](qwen-judge.md)を参照。実運用品質・意味adapter・CPU SLOは未受入。
