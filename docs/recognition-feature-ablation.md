# R2 POS・文字種の条件比較（2026-10-05）

`recognition_quality`にoffline専用の`--feature-ablation`を追加。既存RecognitionReportを検証し、参照とのID/原文/source/document照合を行った後、5条件の該当数を比較する。runtimeのdecision、risk、入力を変更せず、推論・辞書ロードもしない。goldは事後集計だけで使用する。

```powershell
cargo run --locked --no-default-features --example recognition_quality --offline -- references.jsonl observations.json result.json --feature-ablation
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev/evaluate-feature-ablation.ps1 -Offline
```

後者は既存の`target/dictionary-pos-matrix/{small,core,full}/recognition-*.json`を使う。事前に`evaluate-dictionary-matrix.ps1 -Offline -WithPos`で生成する。入力report/hashは各結果へ記録する。ユーザー由来の詳細はtargetのみ、runnerはCI実行を拒否。CIでは既存verifyのrecognition_qualityテストとして人工契約のみ実行する。

## 条件と欠測

| probe | 条件 |
| --- | --- |
| sparse_conjunction | OOV>0かつ未観測文字bigram>0かつ未観測語bigram>0。既存実験policyの統計条件のみ |
| unseen_pos | 未観測POS bigram>0 |
| class_change | 文字種遷移>0 |
| sparse_and_pos | sparse_conjunction AND unseen_pos |
| pos_and_class_change | unseen_pos AND class_change |

POSは入力のPOS欠測0・corpusに有効POS pairあり・入力pair数>0のときのみ適用可能。v1資産、POS欠測、学習POSなし、1語以下はunavailable。文字種は観測あり・2scalar以上で適用可能。ANDは両方の適用が必要で、一方falseでも他方が欠測ならunavailable。単独のsparse条件は従来の条件をそのまま再現する。

`feature_ablation.method=offline_feature_probes.v1`。各probeの`flagged/not_flagged/unavailable`は順に確認済み一致・確認済み不一致・未確認の3列。未確認を品質の分母から除外し、確認済み誤りのrecall分母にはunavailableも含める。適用可能coverageを併記し、分母0はnull。flagは仮の条件該当であり、本番review・誤り確定・低リスク判断を表さない。一次warningや候補findingとのORもこの比較には含めない。

既定CLIの出力は維持。指定時だけoptional feature_ablationを追加し、通常の品質summaryは変化しない。条件・methodを変更した場合は比較結果を別versionとして扱う。

## 既存15件の開発観察

20250129/Mode C、人工clean8文の統計assetを用いた結果。確認済み7件（一致3/不一致4）、未確認8件。3辞書は同じ15件の反復評価であり、N=45とはしない。

| 条件 | 辞書 | 該当: 確認済み一致 / 不一致 / 未確認 |
| --- | --- | --- |
| sparse_conjunction | 全3辞書 | 0 / 1 / 2 |
| unseen_pos | small | 3 / 3 / 8 |
| unseen_pos | core/full | 3 / 4 / 8 |
| class_change | 全3辞書 | 3 / 4 / 8 |
| sparse_and_pos | 全3辞書 | 0 / 1 / 2 |
| pos_and_class_change | small | 3 / 3 / 8 |
| pos_and_class_change | core/full | 3 / 4 / 8 |

この集合ではunavailableは0。文字種遷移単独は全件に該当し、POS単独も確認済み正常3件全てに該当する。既存統計条件へのPOS AND追加は該当件数を変えなかった。単純なPOS/文字種条件を本番reviewへ追加する根拠は得られておらず、採用を見送る。POS自体が無効だと結論しない。人工8文の未観測頻度は代表的な言語統計ではなく、現15件はheld-out testでもない。

追加サンプルを自動収集せず、ASRも実行しない。代表clean統計資産と独立OCR/ASR対が必要な段階ではユーザーに別途依頼する。R2全体は進行中で、source profileに基づく証拠解釈、小さなfusion、独立品質受入は未完了。