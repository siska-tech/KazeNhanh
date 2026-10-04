# 軽量統計のreview baseline

2026-10-05。R2の実験用判定。`RecognitionEngine::with_sparse_statistics_review()`で明示的に有効化する。既定の統計観察と保留は変更しない。学習済みfusion、校正済み誤り確率、品質受入済みpolicyではない。

## 判定条件

domain・辞書・解析設定が一致して適用可能な統計資産について、次の全条件が同じsegmentに存在する場合、`sparse_statistics_conjunction` warningを1件返しreviewへ回す。

1. OOV形態素が1件以上。
2. clean corpusに未観測の原文文字bigramが1件以上。
3. 同じcorpusに未観測のdictionary-form語bigramが1件以上。

共起範囲はsegment全体で、三条件が同じ文字位置に重なるとは主張しない。finding spanも全文。元の統計evidenceに局所未観測spanを残し、誤りspanへ変換しない。各特徴は相関し得るため独立確率として加算/乗算しない。riskはnull、naturalnessは未評価、SLM呼出0、低リスク受理はしない。

条件未成立・分母なし・資産欠落・未知domain/辞書不一致は、このpolicyから追加warningを出さない。既存一次warningは保持し、それ以外はundetermined。OOV単独、希少語単独、高confidence単独から合否を作らない。confidenceの意味が不明な入力でもraw値は保持するが、このpolicyはconfidenceの閾値や変換を定義しない。

候補reviewを併用すると、一次warning OR 候補不一致 OR 統計三条件でreviewになる。証拠間の加点・重み学習ではなく、説明可能な運用上のOR統合。

```rust,ignore
let engine = RecognitionEngine::new(analyzer, config)?
    .with_statistics(statistics)
    .with_sparse_statistics_review()
    .with_candidate_disagreement_review(); // 任意
```

policy IDは単独`kzn.recognition.sparse_review.v1`、候補併用`kzn.recognition.sparse_candidate_review.v1`。既存candidate-only policyは保持。report schemaはkzn.recognition.v2のまま。report.validateは統計OOV数とmorphologyの一致、findingの全文span/根拠件数/asset/policy/reason、候補根拠との整合を検証する。欠落・改ざん・偽の低リスク/確率を拒否する。

## ローカル比較

先に既存のdictionary matrixで3辞書と辞書別統計assetを準備する。その後、追加データやモデルを取得せず、同じ15件で差分を保存する。

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev/evaluate-sparse-review.ps1 -Offline
# 単独runnerでは --sparse-review を最後に指定
cargo run --locked --offline --example recognition_samples -- input.jsonl output.json statistics.json EXPECTED_SHA256 contract_fixture --dictionary dictionary.dic --sparse-review
```

CLIでは統計asset/hash/domainを必須とする。core APIでは未接続も受理し、欠測理由を保持して保留する。詳細・品質集計はtarget/sparse-reviewにローカル保存し、CIでユーザーデータの新規比較はしない。CIの契約テストは人工データを使う。

## 限界と次の検証

未知固有名詞・新語・方言・絵文字を含む正常文でも三条件を満たす可能性がある。短い人工8文の統計資産は代表日本語の分布を表さず、今回の比較は機能観察のみ。既存15件に合わせた閾値調整は行わない。既知語だけからなる自然な誤変換はこの条件で検出できない。

品質検証に追加サンプルが必要になったらユーザーへ用途・件数・正常/誤り例・confidence/N-best形式を伝えて依頼する。開発の前提としてASR実行環境を構築したり外部データを増やしたりしない。代表clean corpus、追加文字種/POS特徴、source固有confidence policy、学習fusion、独立評価による採否判断はR2以降の残作業。
## 2026-10-05 観察結果

small/core/full（20250129、Mode C）とも同じ結果。既定review=0/保留15から、opt-inでreview=3/保留12へ変化。追加reviewは`·資產增也寸`、`ムダ使いをーない`、`10日やせろ`。確認済み不一致4件中reviewは1件、他の追加2件は参照未確認で検出成功に数えない。確認済み一致3件へのreviewは0。risk=null、SLM呼出0、raw confidence保持。

small実辞書の既存正常fixtureで`型番ZX-900B`にもreviewが出ることをOCR/ASR両sourceで確認した。`体系キープ`は保留。したがって、今回の三条件と人工assetを既定policyや製品品質として採用しない。確認済み不一致3件の未検出と、正常な型番への誤警報が残る。

検証: core43件（追加3）、source契約・実Sudachi・最小API・依存境界・workspace/legacyを含むverify.ps1 -Offline成功。正常型番の反例を追加した実Sudachi試験も成功。既存15件は3辞書×opt-inと事後品質集計で比較。新規データの取得・ASR実行・SLM推論は行っていない。