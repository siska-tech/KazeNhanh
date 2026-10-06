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
