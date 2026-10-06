# 実画像OCRデータの登録（2026-10-06）

ユーザー提供の`kzn-dataset/dataset-real/kzn-ocr-real.jsonl`（1,025件）を、合成OCRとは別のローカル評価資源として登録した。データ本体・manifest・sources.lock・README・ATTRIBUTION・除外記録・現在の生成runnerの7ファイルを`resources/evaluation/kzn-ocr-real.lock.json`でhash固定する。認識器は提供元の宣言でpure-onnx-ocr 0.2.1 / PP-OCRv6 medium。データや画像をrepo/CIへ取り込まず、再認識は実行しない。

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev/prepare-ocr-real.ps1 -DatasetRoot C:/Users/Shion/Documents/Projects/kzn-dataset -Offline
```

## 構成と監査

| 出典 | 全件 | development | 提供元status・参照の根拠 |
| --- | ---: | ---: | --- |
| NDL行画像 | 646 | 65 | verified。NDL職員の行テキスト。ただし旧字体画像にも新字体の参照 |
| 官公庁PDF | 246 | 25 | verified。電子PDFのテキストレイヤー。画像との全件人手照合ではない |
| Commons写真 | 133 | 133 | unconfirmed。画像からモデルが転記、人手未確認 |
| 合計 | 1,025 | 223 | 宣言verified892 / unconfirmed133 |

元のsplitはtrain447 / calibration177 / test178 / development223。既存のmetadata監査でID、宣言document/origin、非空参照の完全一致hash等のsplit間重複を検査し合格。行のsplitとmanifestも照合する。これらは提供元の宣言と文字列に関する監査で、画像/転記の真正性や独立性全体の証明ではない。今回は画像hashの全件照合や原画像の目視監査を実行していない。

補助的なsource_file集計では、`caa_wp2026_31.pdf`は全4split、`env_tekiou_pamph2018.pdf`はtrain/calibration/testを跨ぐ。同一ページのdocument分割違反ではないが、同じPDFの文体や用語を共有する。未知文書への外部評価を主張するなら資料単位の別holdoutが必要。source_fileのない行について資料単位の独立性を保証しない。合成集合との交差重複監査も未実施。

## 元の宣言とKazeNhanh側の評価用参照を区別

提供元のラベルを変更せず、developmentの全metadataを`development.declared-references.jsonl`へ保存する。`development.inputs.jsonl`はid/source/document_id/text/engine/confidence/confidence_scaleの7フィールドのみで、gold・CER・一致ラベルを除く。認識文字列とconfidence/nullを保持する。

別の`development.review-references.jsonl`は、原文/転記を保ち、`declared_transcription_status`に元のstatusを保存、KazeNhanh側の`transcription_status`は全件unconfirmedとする。これは参照が誤りという判定ではなく、次の条件が未解決なので既存quality runnerで確定precision/recallへ算入しないための扱いである。

- NDL: 新字体の参照と原画像への忠実な転記をraw.v1で同一視しない。字体正規化を採用するなら別versionのcomparison policy、変換表/実装hash、正規化で隠れる認識誤りを定義する。提供元の`match_jitai_normalized`等の真偽値をそのまま正解ラベルにはしない。
- 官公庁PDF: テキストレイヤーと切り出し範囲・可視文字の対応を評価目的に合わせて確認する。スキャン/写真ではなく電子PDFのレンダリング画像という母集団を明示する。
- Commons: 人手未確認の転記をverifiedに昇格しない。写真の見えない/欠けた文字や原資料自身の表記を含む確認が必要。

各行にreference_basisとreview_reasonを保持。提供元の除外記録では、NDLの高CER候補を確認して4件の参照誤りを除外しており、低CER行は全件目視済みではない。認識結果に依存した除外がある集合として、母集団選択の限界も残す。

## 利用する順序

1. 今回はhash固定・metadata監査・development223件の入力/参照分離まで。train/calibration/testはmetadata監査のみで、学習用射影・特徴生成・推論・品質採点には渡さない。
2. 次にcomparison policyと参照の確認範囲を整理し、source_set別のdevelopment観察を行う。合成用のsource mapping/domainや校正を、名前の似た実画像用profileへそのまま転用しない。生成runnerは前回の合成版監査後に更新されており、実画像用の来歴/集約を別に結びつける。
3. 独立評価用の資料group、受入指標、未評価区分を固定してからheldoutを利用する。新字体化や未確認参照に由来する差を、認識risk検出の成功に数えない。

出典・ライセンスは提供元の宣言を保存し、`ATTRIBUTION-real.md`をローカル成果物にも同梱する。再配布の可否を今回独立に検証したものではない。特にCommonsは行ごとに条件が異なるため元のlicense/artist/license_url/page_refを捨てない。raw行は参照側へ保存し、任意metadataをcoreの採点根拠として読まない。

出力は`target/kzn-ocr-real/`。`preparation.json`に出典/split/status件数、補助的なsource_file重複、射影hash、quality_accepted=false、inference_executed=falseを記録する。合成用の固定候補・辞書・閾値は変更しない。
登録時のdevelopment223件には空文字かつconfidence欠測2件、非空かつconfidence欠測0件。前回未評価だった非空欠測条件は今回のdevelopmentでも埋まっていない。外部原本との原文・数値confidence/null照合と、再実行による全射影hash一致を確認した。

## 閾値を持たないsource記述とdevelopment観察（2026-10-06）

`recognition_samples`に`--source-description description.json`を追加した。既存`--source-rule`とは排他で、description schemaは`kzn.ocr.ctc_description.v1`。認識器/版/decoder来歴・profile/domain・入力engine/scaleを照合するが、閾値・判定ruleは持たず、thresholdの追加はunknown fieldとして拒否する。旧source ruleの機能/出力は維持する。

このdescriptionはCTC emitted-token平均のbox文字数加重という明示された方式に限定する。confidenceをSegment粒度、EngineScore、HigherIsBetter、range=[0,1]、calibration=Noneで保持し、raw値は変更しない。nullはMissing/None、0はObserved、範囲外・engine/scale/source不一致は拒否。候補はMissingのまま。confidenceの解釈と、認識正解確率やreview閾値の受入を分ける。

提供元sources.lockのengine commitと、固定した生成runnerのコードを根拠に集約を記述する。実画像sources.lockにはモデルONNXのhashがないため、合成用モデルhashを転記せず、modelは提供元の名称宣言と記録する。過去の生成実行や実際のモデルbytesを独立に認証したものではない。runnerは縦方向のbox配置なら上から下、その他は左から右に並べ、文字数加重平均・総文字数0ならnullを出力する。実画像用decoder IDにこのrunner hashを保持する。

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev/observe-ocr-real.ps1 -DatasetRoot C:/Users/Shion/Documents/Projects/kzn-dataset -Offline
```

登録時のhash監査を再実行し、NDL・官公庁PDF・Commonsをそれぞれ`ja.ocr.real.<source_set>.ctc.v1` profile / `ja.ocr.real.<source_set>.v1` domainへ分ける。group分けに参照側のsource_setとIDだけを利用し、解析runnerへ渡すのはgoldなし7フィールド入力。各groupをsmall/core/fullで観察し、sourceのraw値・欠測・profile/domain、risk未推定、SLM0、low_riskなしを全reportで検証する。

合成用統計asset・融合係数・confidence閾値は使わない。出力は従来のcore観察envelopeで、source-review wrapperではない。quality runnerへは確認待ち参照だけを渡すため、confirmed_count=0、precision/recallはnullである。`observation-summary.json`は処理件数とreview/undeterminedを記録し、正解率を出さない。

### 比較規約の次の境界

- 現profileのraw.v1は忠実な転記との比較を意図する。原画像に旧字体が書かれているなら、それを新字体へ自動修正した参照をrawの正解にはしない。
- NDL既存参照との新字体化後の一致は、採用する場合でも別のversion付き参照一致診断とする。変換asset・空白/句読点規則・非可逆変換で消える差を明記し、忠実な転記の誤り検出と同じラベルにしない。
- PDF/写真の画像との照合も含め、確認済みsubsetを作るときはreviewが出た例だけを選ばず、source/group単位の選定法を先に固定する。モデルによる再転記を人手確認と呼ばない。

この段階の目的は実データで型契約と既存一次観察を動かすこと。自然な誤認識の識別力や、実画像への統計/fusionの汎化を受け入れたわけではない。
### 観察結果

| 出典 | 件数 | review | undetermined |
| --- | ---: | ---: | ---: |
| NDL | 65 | 2 | 63 |
| 官公庁PDF | 25 | 2 | 23 |
| Commons写真 | 133 | 0 | 133 |
| 合計 | 223 | 4 | 219 |

small/core/fullすべて同じ判断件数。review4件は既存の括弧対応ruleによるもので、行断片でも発生し得る。正解未確認のため成功検出/誤警報のどちらにも計上しない。confidenceの数値に基づく閾値判定は0、low_risk0・risk=null・SLM0を維持。669 reportは同じ223件を3辞書で観察したもので、サンプル数を669としない。

検証: runner3テスト（descriptionの欠測/0/範囲/engine・scale・source照合/threshold拒否を追加）、全example all-features/offline check。全669reportのraw/欠測/profile/domain・risk/SLM/low_riskと品質分母を検査。qualityのconfirmed_count=0、precision/recall=null。既存source-ruleの合成development100件は旧reportとhash一致。calibration/testの特徴生成・推論、閾値調整・学習は未実施。