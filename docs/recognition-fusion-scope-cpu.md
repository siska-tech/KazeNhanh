# 固定OOF候補の適用範囲とCPU観察（2026-10-06）

OOF候補を再学習せずに監査した。`resources/evaluation/ocr-synth-oof-candidate.lock.json`に前回10aa425の3辞書の候補・統計資産・development reportのhashを固定。モデル係数・margin境界0・転記規約raw.v1を変えず、既存developmentの全予測と集計を再計算して完全一致を検証する。未校正marginの実験であり、RecognitionReportのrisk/decisionを変更しない。

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev/audit-fusion-synth-1k.ps1 -Offline
```

以前のローカルOOF成果物が必要。入力/source mappingのsnapshotと候補hashを照合してから、`fusion_baseline audit-frozen`で監査し、release版recognition_samplesで100件の逐次処理を辞書ごとに3回、新しいプロセスで実行する。goldを読むのはoffline監査だけで、計測runnerはgoldを含まない入力を使う。各reportは前回のreportとbyte/hash一致を要求。詳細は`target/kzn-ocr-synth-1k/oof/{edition}/scope-audit.json`と`oof/cpu/`のみ保存する。再学習・calibration/test評価は行わない。

## 欠測と空入力の分離

四つの排他的な区分と全件を出力する。空文字はscalar数0（空白文字のみとは別）、confidence値0は観測済み。0件の区分のprecision/recallはnullにする。

| 区分 | 一致 / 不一致 | confidence-onlyのflag一致 / 不一致 | integratedのflag一致 / 不一致 |
| --- | --- | --- | --- |
| confidenceあり・非空 | 66 / 26 | 1 / 18 | 2 / 20 |
| confidenceなし・非空 | 0 / 0 | 0 / 0 | 0 / 0 |
| confidenceあり・空文字 | 0 / 0 | 0 / 0 | 0 / 0 |
| confidenceなし・空文字 | 0 / 8 | 0 / 8 | 0 / 8 |

3辞書共通。非空区分のintegratedはprecision20/22=90.9%、recall20/26=76.9%。全体の28/34という検出には空文字8件が含まれる。欠測が正常例にも現れる母集団や、非空だがconfidenceが欠測する入力での有効性は未評価である。空文字8件も本集合の参照が非空だから不一致と分かるだけで、原資料が空である可能性や全行脱落をtext-onlyで判定できることを意味しない。欠測indicatorを一般的な誤り証明として採用しない。

APIは欠測を受理しても、学習候補の適用が検証されたとは限らない。空/非空、confidence欠測、未知source/domainの保留理由を独立して維持する。未flagをlow_riskへ変換しない。ASRについて今回の性能根拠はない。

## CPU観察

主対象は常駐プロセスの逐次評価と仮置きし、CLI費用を別に記録した。Windows NT 10.0.26200、Intel64 Family 6 Model 186 Stepping 2、論理CPU16、rustc 1.99.0、releaseビルド。実行ファイルhash、commitとdirty状態、入力/辞書hashをローカル記録する。以下は単一PCの観察でSLOではない。

| 辞書 | ロード中央値 ms | 100件CLI wall中央値 ms | プロセスCPU中央値 ms | 常駐call p50 / p95 ms | 観測peak working set MiB |
| --- | ---: | ---: | ---: | ---: | ---: |
| small | 128.1 | 215.1 | 171.9 | 0.153 / 0.301 | 155.3 |
| core | 220.2 | 357.1 | 281.3 | 0.144 / 0.278 | 267.9 |
| full | 363.3 | 475.8 | 406.3 | 0.148 / 0.269 | 437.4 |

- ロードはSudachi設定/hash検証・engine作成・統計asset読込/hash検証を含む。ファイルシステムcacheは制御しておらず、cold bootの測定ではない。
- 常駐callはcore評価、report検証、source combine/検証を含む。入力adapter構築・JSON入出力・logistic margin計算を含まないため、融合pipeline全体のレイテンシとは呼ばない。各プロセスの初回callを除く99件×3回=297計測のnearest-rank p50/p95。100件の異なる文章を繰り返した測定で、独立した297サンプルではない。
- wallは起動・adapter・JSON入出力・観測側pollingを含む。CPU時間とは別。単発1件CLI、同時実行、長文、SLM、モデル推論は未計測。
- メモリは5ms間隔で実行中に読めたWindows peak working setの最大値。終了直前のpeakを逃す可能性があり、ライブラリ単独のRSS上限ではない。取得不能はnullで0にしない。終了後の取得は0を返したため採用しなかった。最大観測thread数は各辞書4、thread数の強制上限ではない。

smallは今回の統合判定件数が同じでロード/メモリ費用が小さく、次の固定評価の第一候補にできる。ただし観測差の小さいcall速度を辞書間の優劣と断定せず、3辞書比較を保持する。

## 独立評価前の受入ゲート

ここでは順序と必須報告を固定し、未合意の数値を達成済み条件として作らない。

| ゲート | 固定する内容・完了条件 | 現状 |
| --- | --- | --- |
| 対象 | OCR profile/engine版/集約法、raw.v1、segment不一致。review候補の検出と、転記正解確率/低リスク受理を分離 | 合成OCRの固定候補のみ。実OCR/ASRへの適用は未受入 |
| 候補凍結 | input/model/statistics/dictionary/source mapping hash、特徴順、欠測方式、境界0をtest前に固定。予測再現不一致0、契約違反0 | 今回実装・検証済み |
| 品質 | confidence-onlyとの同一testでのpaired比較。空入力を含む全体と非空を別報告。precision/recall、正常例flag率、重大誤り見逃し、分母とgroup単位の不確実性を報告 | 許容誤警報上限・最低recall・重大誤りの定義/重みは運用要件として未決。testは未開封 |
| 証拠不足 | missing+nonempty、正しい空入力、未知source/domain、候補なしの自然な誤認識を対照に含める。例がない区分を合格にしない | 現集合にない区分は未評価。必要な追加例はユーザーへ依頼 |
| CPU | hardware/常駐orCLI/長さ/並列数/threadsを固定。ロード・warm p50/p95・peak・1件/batch・最終融合処理を測る | 特徴/source経路の100件逐次観察まで。latency/RSS上限は未決 |
| 校正/低リスク | 別calibration artifact、対象母集団、必要coverage、最低受理率とlow_risk内誤り率の上限をtest前に固定 | 未実装。今回のflagをlow_riskへ反転させない |

品質/CPUの数値要件と未評価区分が残るため、test開封や本番採用はまだ行わない。次は実運用の対象と許容損失を決め、最終融合処理の計測と、欠測/空入力を含む評価データの充足を確認する。既存200件のtestで合成画像内の比較は可能だが、実画像/ASRの受入を代替しない。追加データを勝手に収集・認識しない。