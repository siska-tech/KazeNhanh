# 提供OCRサンプル: user-002 / user-003

2026-10-04。今回の10認識文・confidenceを[画像2](../evaluation/ppocrv6-medium-user-002.jsonl)と[画像3](../evaluation/ppocrv6-medium-user-003.jsonl)へ各5件保存。前回と合わせ3画像/15件。同一画像の行/列は同じdocument IDを持つ。OCRエンジン名は前回のPP-OCRv6 mediumを継承したmetadataで、ここでOCRを再実行した結果ではない。

## 画像の転記と差

| 画像・位置 | 認識文 | confidence | 画像内容の転記 | 差 |
| --- | --- | --- | --- | --- |
| 2・行1 | 無駄遣いをしない | 0.980 | 無駄遣いをしない | 一致 |
| 2・行2 | ・アンチエイジング" | 0.930 | アンチエイジング | 余分な末尾quote |
| 2・行3 | 体系キープ | 0.877 | 体系キープ | 一致、ユーザー確認済み |
| 2・行4 | ·資產增也寸 | 0.834 | 資産増やす | 文字の置換 |
| 2・行5 | 家族を大事にする | 0.979 | 家族を大事にする | 一致 |
| 3・右列1 | 毎日笑顔で楽しく過す | 0.952 | 毎日笑顔で楽しく過ごす | 送り仮名の差 |
| 3・右列2 | ポイカツを頑張る | 0.876 | ポイカツを頑張る | 一致 |
| 3・右列4 | 家をきれいに保つ | 0.991 | 家をきれいに保つ | 一致 |
| 3・右列3 | ムダ使いをーない | 0.975 | ムダ使いをしない | し→ー |
| 3・右列5 | 10日やせろ | 0.659 | 10kgやせる | 単位と末尾字、ユーザー確認済み |

体系/体型、やせろ/やせるの曖昧箇所はユーザーに確認した。転記候補は履歴として残し、確定したtranscriptionを区別する。他8件は画像からの転記でuser-confirmedとは表示しない。体系を意図した体型へ訂正しない。過す/過ごすの画像忠実性の差と、自然さ/意味の異常を同じlabelにしない。

認識文をそのまま入力し、画像の箇条書き記号を除いた内容の忠実性を比較するpolicy=ignore_leading_bulletを宣言する。認識文にある先頭・/·だけはlabel比較で無視し、末尾quote、本文、report原文は変更しない。bbox座標は捏造せず、行/列位置をmetadataとして保存。添付画像そのものはrepoへ保存していない。

## 投入

```powershell
cargo run --locked --offline --example ocr_samples -- evaluation/ppocrv6-medium-user-002.jsonl target/ocr-002-primary.json
cargo run --locked --offline --example ocr_samples -- evaluation/ppocrv6-medium-user-003.jsonl target/ocr-003-primary.json
```

既定はモデル不要の一次評価、model forward=0。Qwen比較を明示的に行う場合だけ、features qwenと末尾--compare-qwenを指定する。以前の画像1の実測と比較できるよう旧schemaのサンプルも読み込める。参照転記・期待labelはdetector/judge inputへ渡さない。

今回の2画像には、転記との不一致5例（うち送り仮名差1例）がある。現primaryは全10件をacceptable、二次候補0とした。転記不一致5件のgate通過を記録し、ユーザー確認済みと未確認の集計を分ける。これはOCR忠実性の観察で、acceptableが画像との一致を保証するわけではない。confidence=0.975の文字置換例もあり、OCR confidenceを校正済正解率としない。

同一画像の例へthresholdを後付け適合せず、development observationとして比較/評価label設計に使う。新10件のSLM再推論は行っていない。
[観察結果JSON](../evaluation/ppocrv6-medium-user-002-003-observations.json)に、原文/confidence保持・既定SLM呼出0・転記確認状態を記録。転記差5件のうちユーザー確認済みは1件、他4件は未確認転記に基づく暫定比較。

検証: 3データセットの既定runner実行、原文/confidence/document ID保持、SLM呼出/forward=0、featureなしQwen比較の拒否、qwen付きexample compile、workspace tests・cargo fmt・git diff --checkが成功。
