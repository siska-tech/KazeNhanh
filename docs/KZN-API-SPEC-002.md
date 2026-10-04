# KZN-API-SPEC-002: テキスト評価契約

> 2026-10-04追記: 本書は現行kzn.evaluation.v3の契約。新しいrecognition risk APIは[再設計案002](KZN-REDESIGN-PLAN-002.md)で別schemaとして提案している。現acceptableを新low_riskへ自動変換しない。R0は[recognition API](recognition-api.md)として追加済み。confidence/candidatesとrisk推定は未実装。

2026-10-04 / P1〜P3制御契約。新しいRust APIの契約。既存0.1の要約APIとは別の意味を持つ。

## 入力と原文

`TextInput::new(text)`は借用したUTF-8原文をそのまま保持する。languageの既定はja。sourceはPlainText/Ocr/Asr/Llm/Form/Other。domain、reference、原文spanと対応するsource annotationsは任意。bbox・時刻等は呼出側が供給し、coreは推測しない。

`ByteSpan`は原文byte半開区間。newとvalidateは順序・上限・UTF-8文字境界を確認する。空spanは挿入位置等の表現用に許容し、形態素には非空spanを要求する。全文trim・正規化・文分割による位置変更は行わない。

日本語以外、不正span、max_input_bytes超過はEvaluationError。空文字は正常受理できる入力であり、required formの違反判定はP2のprofile/ruleが担う。

## エンジンと結果

`EvaluationEngine::new(Arc<dyn MorphAnalyzer>, EvaluationConfig)`はモデルを必要としない。`evaluate(TextInput)`はResult<Report, Error>、`evaluate_batch(&[TextInput])`は入力順のVec<Result<Report, Error>>を返す。各入力の失敗は他入力を中断しない。

MorphAnalyzerは全形態素を出現順で返す。coreは原文surface・byte span・非重複を確認し、backendが正規化文をsurfaceとして返した場合はエラー。PrimaryDetectorを設定しないEvaluationEngine::newは全文を未評価として返す。P2の標準japanese_engineはprofile付きPrimaryRulesを注入する。形態素解析の成功だけを正常判定としない。

Reportはoriginal_text/source/domain/annotations、schema_version=`kzn.evaluation.v3`、feature_version=`kzn.morphology.v1`、profile、三軸score、issue、routing、coverage、limitations、provenance、metricsを持つ。訂正文のフィールドは持たない。参照本文は結果へ自動複製しない。

## Score・保留

validity/naturalness/semantic_consistencyは共通DimensionScore型。valueは0..1のUnitScoreまたはnull。高いほど良い。NaN・無限大・範囲外を拒否する。未評価・文脈不足・対象外・失敗はnullで、methodやconfidenceを付けない。calibrated methodにはcalibration_idが必須。他methodにcalibration_idを付けない。

semantic scopeはinternal/reference。参照評価を要求したのに文脈が欠落/空ならinsufficient_context。必須軸の未評価があるacceptableはundeterminedへ落とす。heuristicやSLM自己申告値を校正済確率として扱わない。

coverageは各軸について全文の評価済/未評価spanを明示する。部分評価はまだサポートしない。P2の一次候補/未解決必須軸はsecondary_needed=true/status=disabled、確定invalidや解決済screeningはfalse/not_requested。detector未設定はnull。slm_calls=0。

## JSONと検証

全enumはsnake_case。未知のreport fieldを拒否する。JSON deserializeだけでは原文とのspan対応を保証できないため、`EvaluationReport::validate()`でschema/score/必須軸/coverage/spanを検証する。未知schemaを黙って受理しない。

API Errorにはgit2、Candle、Sudachiの固有型を含めず、backend名と診断messageを返す。入力文章の問題はIssue/Reportで返し、資産ロード・backend contract違反はErrorで返す。

## 0.2標準backend

japanese_engine(SudachiConfig, EvaluationConfig)でSudachiを組み立てる。SudachiConfig::from_pathsまたはowned dictionary/settings bytesを指定し、Mode A/B/Cを選ぶ。rootのdefaultはSudachiのみ、no-default-featuresでは任意MorphAnalyzerを注入する。旧APIはlegacy featureへ分離。[具体例と移行](migration-0.2.md)。

## P2一次判定

標準profileの必須軸はvalidity/naturalness。意味軸は未評価で合格へ変換しない。ja.llm.v1はreference scopeと意味軸を必須にする。profile_config・metrics.morphologyを含むschema v3へ更新。[ルール・scoreの範囲とCLI](primary-detection.md)を参照。

P3制御契約: optional SecondaryWorkerを明示接続すると選択的judgeを実行できる。標準CLIはworker未接続。reference不足はcontext_missing/呼出0、不正出力・予算・timeoutは保留。[接続・制約・実モデル残作業](secondary-judging.md)。

P3実験用adapter: optional qwen feature/crateでCPUの自然さpaired-label判定を追加。固定資産セットアップと独立token ID/CPU smokeは[Qwen手順](qwen-judge.md)を参照。実運用品質・意味adapter・CPU SLOは未受入。
