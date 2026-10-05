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
## 学習例を除外した統計特徴（2026-10-06）

`evaluate-oof-synth-1k.ps1`で5-foldの統計特徴を生成する。trainだけをdocument_id・manifestのorigin_id・非空転記完全一致の推移的な連結成分にまとめ、最小ID順に件数の少ないfoldへ割り当てる。正誤ラベルで層別せず、同一groupを分割しない。各foldの統計資産は残る4foldのclean転記だけから作り、除外したfoldの認識結果を特徴化する。developmentにはtrain全体の統計資産を使う。今回のtrain500は各100件の5fold、各統計資産400件、development用500件となった。

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev/evaluate-oof-synth-1k.ps1 -DatasetRoot C:/Users/Shion/Documents/Projects/kzn-dataset -Offline -UsePreparedSnapshot
```

通常はsource runnerを通して外部ファイルの固定hashとsplitを再監査する。今回は外部READMEに実画像版の案内が増え、生成runnerに縦方向の行順/空白処理が追加され、旧hashと不一致になった。データ本体・manifest・sources.lock・ATTRIBUTIONは変更なし。新しい生成コードで古いデータの来歴を上書きせず、`-UsePreparedSnapshot`で以前監査したtrain/development射影とsource mappingの固定hashを照合して再利用した。固定値は`resources/evaluation/kzn-ocr-synth-1k-oof.snapshot.json`、原データ4ファイルのhashは既存lockを使う。旧runnerのhashもmapping内に保持する。詳細データ・snapshot本体はtarget内のみ。新しい実画像データはこの評価へ追加していない。

`fusion_baseline prepare-oof`はtrain参照とmanifestからplan、clean corpus、goldを含まない認識入力を作る。`merge-oof`は5foldのreportを検証し、重複拒否・f64往復精度を保って統合する。学習CLIに`--oof-plan plan.json`を付けると、参照hash・group割当・補集合・corpus hashを再計算し、各train reportが自分を除外したfold資産を使ったこと、developmentが全train資産を使ったことをcorpus ID/hash/件数で照合する。この検証後だけfold間の資産ID差を許容し、解析器/source rule等の同一性は維持する。planのorigin宣言は入力manifestに依存し、standalone CLIだけで外部manifestの真正性を保証するものではない。上記runnerは固定manifest hashを別途検証する。

OOFに変更するのは統計特徴の構築のみ。モデル・標準化方式・学習回数・L2・margin境界は前回と同じ。各train例は自分を除外した統計特徴を持つが、logistic係数自体は全train500件で学習する。trainのOOF予測性能を独立test性能として報告するものではない。400件と500件の統計規模差、テンプレート間の依存、合成画像という限界は残る。
### OOF結果

| 特徴 | 辞書 | flag一致 / 不一致 | precision | recall |
| --- | --- | --- | --- | --- |
| confidence_only | 全3辞書 | 1 / 26 | 26/27 ≈ 96.3% | 26/34 ≈ 76.5% |
| text_only | small | 6 / 25 | 25/31 ≈ 80.6% | 25/34 ≈ 73.5% |
| text_only | core/full | 6 / 26 | 26/32 ≈ 81.3% | 26/34 ≈ 76.5% |
| integrated | 全3辞書 | 2 / 28 | 28/30 ≈ 93.3% | 28/34 ≈ 82.4% |

正常例66件の統合flagは前回64件から2件へ減った。smallの正常例の未観測語bigram率平均は、OOF train301件で0.73845、development66件で0.72621（前回trainは0）。特徴構築時の分布差は縮まった。confidence_onlyとの比較ではsmallの統合flagに不一致2件・一致2件が新たに加わり、一致1件が外れた。不一致の取りこぼしも6件残る。総件数だけから一方の集合が他方を含むとは扱わない。

developmentを用いた設計修正後の結果であり、**本番採用・品質受入は引き続き未実施**。正しく読めた確率やlow_riskへ変換しない。SLM呼出0、runtime policy変更なし。calibration/testの特徴生成・推論・閾値調整なし。次はこの候補を固定して欠測/空入力等の適用範囲とCPU費用を整理し、独立評価前の受入条件を定義する。追加Nが必要になればユーザーへ依頼し、自動収集・ASR実行は行わない。

検証: fusion7テスト（OOF追加3）、全example all-features/offline check、3辞書matrix、small再実行byte/hash一致、PowerShell構文とformat/diff。詳細出力は`target/kzn-ocr-synth-1k/oof/`、集約は`summary.json`。