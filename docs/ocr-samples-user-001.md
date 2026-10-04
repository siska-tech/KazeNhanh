# 提供OCRサンプル: PP-OCRv6 medium / user-001

2026-10-04。ユーザーが提供した5件の認識文とconfidenceを[JSONL](../evaluation/ppocrv6-medium-user-001.jsonl)へ保存。単一の手書き画像・縦書き5列由来。document_idはuser-image-001で共通とし、今後のtrain/calibration/test分割で同じ画像の列を別集合へ分散させない。用途はdevelopment observation。品質受入test集合へ読み替えない。

認識文を正規化・訂正せず投入する。confidenceは全文spanのSourceAnnotationへocr_confidenceとして保存し、評価score/校正済確率と混同しない。今回confidenceを理由に自動invalid/SLM routeにはしない。bbox座標は未提供のため捏造せず、画像の列位置だけをimage_column_from_rightで記録する。元画像はチャット添付を参照しており、画像ファイルのrepo保存・hash確認は行っていない。

画像からの確認済み転記:

| 認識文 | confidence | 画像原文 | 右からの列 |
| --- | --- | --- | --- |
| 、運動之始め了 | 0.596 | 運動を始める | 4 |
| 節約する | 0.767 | 節約する | 3 |
| 規則正しい生活毛可 | 0.763 | 規則正しい生活をする | 5 |
| 健康的になる | 0.880 | 健康的になる | 2 |
| 則金とする | 0.622 | 貯金をする | 1 |

ユーザーが転記を確認済み（2026-10-04）。transcription_status=user_confirmed_2026-10-04。3件のOCR不一致、2件の一致を画像原文に対して記録する。OCR不一致と自然さの異常は同じlabelではない（流暢な誤認識はあり得る）。

```powershell
cargo run --release --locked --offline --features qwen --example ocr_samples -- evaluation/ppocrv6-medium-user-001.jsonl target/ppocrv6-medium-user-001-results.json
```

runnerはOCR sourceとannotationを保持したcascadeを測り、その後、別のjudgeを使う明示的なオフライン全件比較を測定する。全件比較を本番routingに接続しない。画像原文・期待labelをprimary/model inputへ渡さず、参照情報の漏洩を防ぐ。結果にはcascadeの一次根拠・二次呼出数・OCR不一致のgate通過数・比較model scoreを保存する。未対応semantic軸は評価しない。

この5件だけでthresholdやconfidence gateを調整して品質保証しない。実際の認識誤りをgateが捉えられるか、固有名詞/短文の正常例を異常化しないかを、追加の独立画像・用途別labelで検証する。
## 初回の実測結果

同じ原文を使うcascadeは全5件をacceptable/secondary_needed=falseとし、SLM呼出0。画像原文との不一致3件が二次候補にもならなかった（この画像におけるgate見逃し3/3）。acceptableは実装済みscreening範囲だけの判定で、OCR忠実性を保証しない。

明示的offline全件比較のQwen naturalness:

| 認識文 | 画像原文との一致 | model score |
| --- | --- | --- |
| 、運動之始め了 | 不一致 | 0.99911 |
| 節約する | 一致 | 0.99995 |
| 規則正しい生活毛可 | 不一致 | 0.99997 |
| 健康的になる | 一致 | 0.99982 |
| 則金とする | 不一致 | 0.99975 |

暫定閾値0.5でmodelも不一致3件を自然さ異常として検出できなかった。自然さと画像忠実性は別の評価なので、これだけで全OCR誤りを自然さlabelにしないが、この候補をOCR誤認識検出として採用する根拠にはならない。品質未受入を維持する。confidence（0.596〜0.880）を校正済の正解率へ読み替えず、同一画像5件への後付けthreshold最適化をしない。

このデータは、一次通過側を含むmodel/判定方式比較と、後続の用途別label設計に活用する。[集計](../evaluation/ppocrv6-medium-user-001-observations.json)は手元CPUでの観察結果で、他の画像の性能を保証しない。