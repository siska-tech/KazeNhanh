# Recognitionのオフライン品質集計

2026-10-05。R2の比較基盤。`recognition_quality`は既に生成されたRecognitionReportと参照転記を照合し、segment単位の転記差とdecisionを集計する。モデル・辞書・認識engineをロードせず、参照を推論へ渡す経路は持たない。検出器の変更や品質受入ではない。

```powershell
cargo run --locked --offline --no-default-features --example recognition_quality -- evaluation/ppocrv6-medium-user-001.jsonl target/dictionary-matrix/small/recognition-001.json target/quality-001.json
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev/evaluate-dictionary-matrix.ps1 -Offline
```

matrixはsmall/core/fullの各editionで`quality-001.json`〜`quality-003.json`をローカル保存する。ユーザー由来のレポートはCIへアップロードしない。CI/verifyでは人工の集計・照合契約テストだけを実行する。

## 入力・照合

- 参照JSONL: `id`、`text`、`transcription`、`transcription_status`が必須。`document_id`、`source`、`comparison_policy`を指定できる。既存OCR fixtureとの互換性のため、document省略時のみ`user-image-001`、source省略時のみ`ocr`。新しいOCR/ASR集合では明示する。
- 入力レポートは`kzn.recognition.observation.v1`の`reports`配列。全RecognitionReportをvalidateし、segment IDで一対一対応させ、原文・source・documentを完全一致で確認する。未知schema、不正report、重複/不足/余分なID、原文差を拒否する。入力summaryの集計値は信用せず再計算する。
- statusは既存fixtureの`user_confirmed_2026-10-04`と汎用`verified`だけを採点対象とする。`image_transcription_unconfirmed`/`unconfirmed`はdecision別の未確認件数に残し、品質の分母から除く。他のstatusは明示的な変換を要求して停止する。`verified`の付与には原資料確認が必要であり、評価器自身が確認するわけではない。
- `raw.v1`（省略時）はUnicode正規化をしない完全一致。`ignore_leading_bullet.v1`は先頭の`・`/`·`/`•`を最大1文字だけ両側から除く。旧fixtureの`ignore_leading_bullet`はこの版に対応させる。空白、数字、単位、送り仮名、句読点、異体字は変更しない。未知policyは拒否する。
- `ocr_mismatch_expected`やその他の付加metadataは採点に使わず、確認済み転記との差からラベルを計算する。comparison policy別件数と入力2ファイルのSHA256を出力し、後から比較条件を確認できる。runtimeの転記policyとは独立した、オフライン比較規約である。

入力は各64 MiB、参照は最大4,096件。出力schemaは`kzn.recognition.quality_observation.v1`。追加依存や公開core API変更はない。

## 指標の意味

decisionごとに「確認済み一致・確認済み不一致・未確認」の3列を保存する。確認済み集合だけで次を計算し、分母0はnullにする。

| 指標 | 定義 |
| --- | --- |
| review precision | reviewされた不一致 / 確認済みreview |
| review recall | reviewされた不一致 / 確認済み不一致 |
| review false positive rate | reviewされた一致 / 確認済み一致 |
| undetermined rate | 保留 / 確認済み全件 |
| low-risk coverage | low_risk / 確認済み全件 |
| low-risk error rate | low_risk内の不一致 / 確認済みlow_risk |
| mismatches not reviewed | 保留またはlow_riskに残った確認済み不一致数 |

保留をtrue positiveにも低リスク受理にも数えない。low_riskが0件ならerror rateは0%ではなくnull。CER/WER、span品質、校正指標、信頼区間は算出しない。全件reviewも誤警報率とprecisionに現れる。

## 提供OCRでの観察と残作業

提供15件は確認済み7件（一致3、不一致4）と未確認8件に分かれる。small/core/fullすべてでreview=0、undetermined=15、low_risk=0。確認済み不一致4件は全て保留に残り、review recall=0%、保留率=100%、low-risk coverage=0%。review precisionとlow-risk error rateは分母0のためnull。原資料3画像内の相関があり、独立testやASR品質の根拠にはしない。出力は常に`quality_accepted=false`、`development_observation_not_held_out_test`を明示する。

独立した実OCR/ASR集合、文書/話者単位のsplit、代表domainでのquality/CPU受入、text/source/統合ablationと統計的な不確実性の評価はR2以降の残作業。今回の集計結果に閾値を合わせ込まない。