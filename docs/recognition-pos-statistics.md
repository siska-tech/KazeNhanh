# POS bigram統計

2026-10-05、R2の追加特徴。既存の形態素解析1回から、隣接する全POS vectorの頻度・未観測率・原文byte spanを抽出する。語彙にある単語同士でも語列の出現状況を比較できるようにするための観察値であり、文法の正誤や誤認識確率ではない。

## APIと互換性

`StatisticsArtifact::fit_with_pos(metadata, documents)`で`kzn.statistics.v2`資産を生成する。既存の`fit`はv1生成を維持し、v1資産/旧観察JSONは引き続き読み込める。builderは最後に`--with-pos`を指定する。資産IDは辞書identity digestに加えstatistics.v1/v2を区別する。

v2の`pos`はpair_count、missing_pair_count、pairs（疎頻度表）。POS vector2個をcanonical JSON配列にしたkeyを使う。Sudachiの場合、品詞・細分類・活用情報を含むvector全体が対象。同じ表層でもタグが変われば別のpairになる。辞書/解析mode/backend/domainの一致条件は既存統計と同じ。

v1はposなし、v2はpos必須。schema/posの矛盾、非canonical key、不正頻度、合計/coverage不整合を拒否する。全4表合計で100,000 entries、key最大4,096 bytes。学習側のPOS利用可能pair数と欠測pair数の和は語bigram総数に一致する。

公開Rust型StatisticsArtifact/StatisticsObservationにはoptionalなposフィールドを追加した。構造体literalを直接書く利用側は旧動作なら`pos: None`を追加する。fit/fit_with_posを利用するコードは変更不要。RecognitionReportのschemaはv2を維持し、lexical_statistics内にoptionalなPOS観察を追加する。旧readerは新POS payloadを拒否し得るため、v2統計を使う場合はreaderも更新する。

## 欠測と範囲

- 空vector、先頭`*`、空/空白タグ、16要素超過、128 bytes超過のタグはこの特徴では使用不能。隣接pairの片側でも使用不能ならmissing_pair_countへ加える。
- 欠けたtokenを飛び越えない。文書/segmentを跨ぐpairも作らない。
- それ以外の`*`は細分類等の未指定タグとして保持する。独自の品詞変換・正規化は行わない。
- 観察のavailable_pair_count＋missing_pair_countは入力語bigram数に一致する。
- 学習側のPOS pairが0ならfrequencies=null。入力側にPOSがあっても100%未観測に変換しない。
- 学習側にPOSがあり入力pairが0なら、frequenciesはunit_count=0/unseen_fraction=null。観測0%と区別する。
- 未観測位置は最大64件、省略数と全体件数を保持する。spanは元の2形態素を囲み、間に空白等があれば含む。

POSの未観測率からreview/riskを自動生成しない。前工程の実験用sparse reviewも従来のOOV＋文字/語bigram条件のまま。頻度が低い品詞列にも正常な口語・省略・固有名詞があり、採用する判定条件は別途評価が必要。

## 3辞書比較

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev/evaluate-dictionary-matrix.ps1 -Offline -WithPos
```

既存の人工clean 8文から各辞書のv2資産を生成し、同じ提供OCR15件を観察する。出力はtarget/dictionary-pos-matrix、従来v1のtarget/dictionary-matrixを上書きしない。small/core/fullは同版20250129・Mode C。summaryの各case/editionのpos_pairsから欠測/未観測を比較できる。

verify-statisticsはv1/v2双方の再現生成を確認。CIでユーザー由来の追加レポートを公開せず、人工契約を検証する。今回も追加データ取得・ASR実行・SLM推論は行わない。人工8文の分布は代表品質を保証しない。品質用N増しが必要な段階ではユーザーへ用途・件数・形式を伝えて依頼する。
## 2026-10-05 ローカル観察

| 辞書 | 入力POS pair数 | POS欠測pair数 | コーパス未観測pair数 |
| --- | ---: | ---: | ---: |
| small | 46 | 0 | 34 |
| core | 43 | 0 | 33 |
| full | 43 | 0 | 33 |

提供OCR15件、人工clean 8文のasset。全辞書で既定review=0/保留15、risk=null、SLM呼出0。未観測数は誤り数・recallではない。v1のsmall assetは変更前とbyte/hash一致を確認した。

検証: POS追加5契約（core全48件）、v1/v2再現生成、3辞書matrix、既存のsource/最小API/実Sudachi等。並行実行中のEXEに対するWindowsリンクロックで最終workspaceビルドが1度失敗したため、辞書比較終了後に同じworkspace試験を単独で再実行し成功した。