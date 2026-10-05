# 軽量学習fusionのdevelopment比較（2026-10-06）

モデルruntimeへ採用する前のoffline比較として、標準化した特徴に対する小さなlogistic線形モデルを実装した。生成SLM・訂正は使わず、新規依存は追加しない。出力は未校正marginと仮のflagで、RecognitionReportのrisk/decisionを変更しない。

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev/evaluate-fusion-synth-1k.ps1 -DatasetRoot C:/Users/Shion/Documents/Projects/kzn-dataset -Offline
```

先にevaluate-ocr-synth-1k.ps1でtrain限定統計assetを作成する。fusion runnerはsource比較runnerで固定hash/split/統計学習集合の照合を再実行し、train500件の認識結果を3辞書で特徴化する。calibration/testを推論や学習には渡さない。詳細はtarget内のみ、CIは人工契約テストだけ。

## 固定した比較条件

- confidence_only: source confidenceとその欠測。
- text_only: log(1+scalar文字数)、OOV率、未観測文字bigram/語unigram/語bigram/POS bigram率、文字種遷移率。
- integrated: 上記すべて。

各特徴はtrainの観測値だけの平均・標準偏差で標準化。欠測は平均へ補完して標準化値0とし、別の0/1欠測indicatorを常に加える。値0と欠測は区別する。全欠測・定数列のscaleは1。モデルの学習に必要な補完であり、RecognitionReportの欠測を観測へ変更しない。

ラベルはverified/raw.v1の認識文字列と転記の不一致。trainの両クラスが必要。全batch・500step・学習率0.05・L2=0.01（interceptを除く）・class weightingなし・初期重み0。学習後のmargin>=0をflagとする。これらをdevelopment評価前に固定し、結果に合わせた調整はしない。学習器内部のsigmoidを校正済み認識誤り確率として出力しない。

CLIは`fusion_baseline train.references.jsonl train.reports.json development.references.jsonl development.reports.json output.json`。source wrapperをvalidateし、ID/原文/source/documentを照合。split名・verified/raw.v1を要求、統計asset・解析資産・source ruleの同一性とtrain/developmentのID/document/非空原文転記の重複を検査する。入力64 MiB、各集合4096件を上限とする。共有テンプレート等の独立性は宣言/完全一致監査だけでは保証しない。

標準化/重みをtrainだけから学習し、その後development特徴とラベルを読み込む。入力hash、特徴順、欠測規約、学習設定、source/統計signature、モデル係数、各件margin/flagを保存する。再実行のbyte/hash一致をsmallで確認した。

## 観察結果

trainは一致301/不一致199、developmentは一致66/不一致34。

| 特徴 | 辞書 | flag一致 / 不一致 | precision | recall |
| --- | --- | --- | --- | --- |
| confidence_only | 全3辞書 | 1 / 26 | 26/27 ≈ 96.3% | 26/34 ≈ 76.5% |
| text_only | small | 65 / 34 | 34/99 ≈ 34.3% | 100% |
| text_only | core/full | 64 / 34 | 34/98 ≈ 34.7% | 100% |
| integrated | 全3辞書 | 64 / 34 | 34/98 ≈ 34.7% | 100% |

**この融合モデルは本番採用しない。** text_only/integratedはほぼ全件にflagを出しており、recall100%を性能改善と扱わない。confidence_onlyも合成画像developmentの結果で、校正・独立test受入ではない。前回のラベルなし下位10%境界と異なり、今回のconfidenceモデルはtrainの正誤ラベルを用いて学習したもの。

原因候補として統計特徴の構築時の分布差を確認した。smallの一致例における未観測語bigram率の平均はtrain301件で0、development66件で約0.726。trainの参照自身が統計資産に含まれるため、学習時には正常例の語列が既知だが、development正常例の多くは未知になる。この差は実測されたが、モデルの全誤警報を単一原因へ断定しない。

次は原文/document groupを単位としたout-of-fold特徴生成、または学習器の例と独立したclean統計集合を比較する。前者は各学習例を含まない統計資産でその例の特徴を計算する。今回の出力を採用済artifactとせず、testを見ずに特徴構築を直す。データ増量・ASR実行は今回不要。runtime policyは既存のまま。