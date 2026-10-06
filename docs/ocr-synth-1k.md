# kzn-ocr-synth-1k登録（2026-10-06）

ユーザー提供の外部datasetを今後のOCR評価集合として登録した。原本は`C:/Users/Shion/Documents/Projects/kzn-dataset/dataset/kzn-ocr-synth-1k.jsonl`。本文・画像はKazeNhanh repoへ複製せず、`resources/evaluation/kzn-ocr-synth-1k.lock.json`にdataset/manifest/README/ATTRIBUTION/sources.lockと生成runnerのSHA256を固定する。

提供元READMEによると、描画した合成行画像をpure-onnx-ocr 0.2.1 / PP-OCRv6 mediumで実際に認識したデータ。data_kind=synthetic、参照は描画文字列、verifiedは提供元の宣言である。実撮影OCRやASRの品質を保証する集合としては扱わない。画像と転記の独立目視検証は未実施。

## ローカル取り込み

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev/prepare-ocr-synth-1k.ps1 -DatasetRoot C:/Users/Shion/Documents/Projects/kzn-dataset -Offline
```

固定hashを照合し、既存validate_recognition_datasetでmanifest/schema/hash/ID・原文グループ・split監査を実行。その後行のsplitとmanifestも照合する。既存の分割を変更しない。出力はtarget/kzn-ocr-synth-1kのみで、CI実行を拒否する。

| split | 件数 | 今後の用途 |
| --- | ---: | --- |
| train | 500 | 統計資産や推定器の学習 |
| calibration | 200 | estimatorと分離した校正 |
| development | 100 | 特徴・条件検討 |
| test | 200 | 条件固定後の評価 |

各splitについて`*.references.jsonl`は出典・ラベルを含む元行を保持、`*.inputs.jsonl`はid/source/document_id/text/engine/confidence/confidence_scaleのみ許可する。transcription、CER、正誤ラベル、生成条件を推論へ渡さない。将来box情報を使う場合は座標契約を別途追加する。ATTRIBUTION.mdもローカル出力へ保存する。Tatoeba/青空文庫/自作等の出典は参照行に保持し、混合ライセンスを一括CC0としない。

1,000件のsplit監査成功。train/calibration/test/developmentは500/200/200/100。再実行のsplit hash一致、全件原文/confidence保持、推論入力キー制限を確認。confidence=nullの78件を0へ変換しない。監査は宣言されたgroupと同一転記hash等の検査であり、類似テンプレート・同作品などの潜在的依存を自動排除しない。

## 評価へ接続する次の作業

登録時点ではKazeNhanh推論・閾値探索は未実施だった。その後nullをtyped missingへ変換するrunner対応とdevelopment比較を追加した（次節）。提供元のconfidence_scaleはCTC emitted-token probabilityのbox文字数重み付き平均との宣言で、転記正解確率ではない。raw値と定義を保持し、confidence ruleのtarget/decoder等を推測で補わない。

trainの参照だけからsmall/core/fullそれぞれの統計assetを作り、developmentで既存rules/統計/候補条件の比較を行う予定。calibration/testの転記を資産学習へ混ぜない。テストを見る前に採用条件を決める。人工画像内の性能と実画像への一般化を区別し、追加の実OCR/ASRが必要になった時点でユーザーに依頼する。画像再生成・OCR/ASR再実行は不要。
## Development比較runner（2026-10-06）

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev/evaluate-ocr-synth-1k.ps1 -DatasetRoot C:/Users/Shion/Documents/Projects/kzn-dataset -Offline
```

固定hash/split監査を再実行し、train.references.jsonlの500件のtranscriptionのみをtrain.clean.jsonlへ写す。small/core/full（20250129、Mode C）それぞれでPOS付きv2統計assetを生成する。混合ライセンス表示と参照行の出典を保持し、domainはこの集合専用ja.ocr.synth.v1とする。

development.inputs.jsonlの100件について、既定policyと既存opt-in sparse reviewを比較。recognition_qualityでraw.v1の確認済み転記との差を集計し、5条件のfeature ablationも出す。条件や閾値をこの結果に合わせて変更しない。calibration/testは取り込み時の分割監査のみで、資産学習・推論・品質集計へ渡さない。

recognition_samplesはconfidence=nullをMissing/score=Noneとして扱い、数値0はObservedとして区別する。confidenceフィールド自体の欠落や文字列/真偽値は拒否。raw定義confidence_scaleはannotationへ保持し、typed scoreの方向・target等を推測しない。入力上限16 MiB/4096件を追加。nullの空認識にspanがない場合も原文の空spanを保持し、原資料全体の完全性を保証しない。

この評価で、頻度比率のJSON往復後に厳密検証が失敗する問題を発見した。coreのserde_jsonにfloat_roundtripを指定し、値の読み戻しを維持する。小数の誤差を許して契約検証を緩める方法は採らず、複数の分母を使った回帰テストで再現/修正を確認した。

詳細report/統計asset/集計はtarget/kzn-ocr-synth-1k内のみ。CIではnull処理と小数往復の人工契約を検証する。実OCR画像での品質受入やASR性能は未評価である。
### Development 100件の結果

raw.v1で一致66/不一致34。全件verified宣言、confidence欠測8件。3辞書とも既定はreview0/保留100、sparseはreview24/保留76（確認済み一致7・不一致17）。sparseの不一致review recall=17/34=50%、precision=17/24≈70.8%、一致へのreview率=7/66≈10.6%。low_riskは0、risk=null、SLM呼出0。これはdevelopment観察であり品質受入ではない。

| 条件 | small 該当一致/不一致 | core 該当一致/不一致 | full 該当一致/不一致 |
| --- | --- | --- | --- |
| sparse | 7 / 17 | 7 / 17 | 7 / 17 |
| POS未観測 | 30 / 18 | 31 / 19 | 32 / 19 |
| 文字種遷移 | 64 / 24 | 64 / 24 | 64 / 24 |
| sparse AND POS | 0 / 10 | 1 / 11 | 1 / 11 |
| POS AND 文字種遷移 | 30 / 17 | 31 / 17 | 32 / 17 |

POS/文字種は単独で不一致の証明にならない。sparseへのPOS ANDは誤警報を減らすが、拾える不一致も減らしている。POSを使う条件はsmall/coreで不一致8件、fullで一致1件＋不一致8件が適用不能（pair不足・空入力等）。文字種遷移も不一致8件が適用不能。適用不能を陰性・正常へ変換しない。既存条件は変更せず、この集合で採用閾値を探索しない。

3辞書×2policyの全600reportについてID/原文/raw confidence/null保持、risk未算出、SLM0を照合した。同じ100件を繰り返した比較なのでサンプル数を600と扱わない。
## Source confidenceのdevelopment比較（2026-10-06）

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev/evaluate-source-synth-1k.ps1 -DatasetRoot C:/Users/Shion/Documents/Projects/kzn-dataset -Offline
```

既存のtrain限定statistics.jsonが必要。固定hash/split監査と、train参照から再構成したclean corpusのhash照合を行ってから比較する。生成runner `ocr-runner/src/main.rs`もlockに追加した。JSONLの認識器文字列・confidence_scale・source=ocrの完全一致でmappingを適用する。旧15件へこのmappingを流用しない。

提供元sources.lockのengine git revision `de050708f98bb3639cb882f538c049408f06173f`にあるCTC実装を読み、blank/repetitionを除いた出力tokenの平均scoreを確認した。dataset runnerはboxごとの認識文字数でこのscoreを重み付けし、全boxの文字数0ならnullを出力する。画像/音声runtimeは実行していない。asset/生成元の宣言と保存コードを根拠とする定義で、過去の生成実行を独立に認証したものではない。

型付きscoreはEngineScore / HigherIsBetter / range=[0,1] / calibration=None、target=emitted-token-confidence.box-char-weighted、granularity=Segment、aggregation=ctc-emitted-mean.box-char-weighted.v1。modelはdet/rec ONNX hash、versionは0.2.1とengine commit、decoderはcommitと生成runner hashに結びつける。転記正解確率にしない。nullは従来どおりMissing/None。

閾値はtrain.inputs.jsonlの観測済みconfidenceだけから、事前固定の「下位10% nearest-rank」を選ぶ。ラベル・正解転記を閾値選択には読まない。rank=ceil(0.1*N)の値を境界とし、strict `<`でreview要求する。これは運用推奨値でも校正でもない。train39件の欠測を除いた461件の47番目、境界は0.76293839301381794となった。method、rank、欠測数、input/mapping hashをsource-summary.jsonへ記録する。

recognition_samplesの末尾オプションは`[--source-rule mapping.json] [--dictionary path] [--sparse-review]`の順。mappingは最大64 KiB、unknown field拒否。明示mappingなしでは従来のunknown score semanticsを維持する。指定時はsource専用観察envelopeを出力し、stdoutの集計はbaseと明記する。baseとconfidenceからSourceReviewReportを構築・検証し、最終decisionをrecognition_qualityで採点する。

### 結果（developmentのみ）

| 判断 | 一致66件のreview | 不一致34件のreview | 保留 |
| --- | ---: | ---: | ---: |
| 既定core | 0 | 0 | 100 |
| confidence OR 既定core | 0 | 9 | 91 |
| 既存sparse core | 7 | 17 | 76 |
| confidence OR sparse | 7 | 17 | 76 |

small/core/fullすべて同じ件数。confidence単独はprecision9/9・recall9/34だが、合成画像developmentの小集合に限る観察であり品質受入ではない。confidenceのreview9件は既存sparseのreview集合内で、ORの追加検出は0。欠測8件はsource判定で適用不能のまま。高confidenceによる既存reviewの取消は0、low_risk0・risk=null・SLM0を維持する。条件/閾値の変更、calibration/testの推論・品質集計は行わない。

次は融合の比較設計を検討する。今回の下位10%境界が最適とは主張せず、この結果を理由にtestへ合わせ込まない。実撮影OCR/ASRへの一般化と校正は未評価。