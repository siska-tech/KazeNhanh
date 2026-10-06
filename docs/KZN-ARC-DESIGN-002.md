# KZN-ARC-DESIGN-002: 評価責務の分離

> 2026-10-04追記: 以下は現0.2の構成。[recognition risk再設計案002](KZN-REDESIGN-PLAN-002.md)では依存分離・Sudachi・worker制御を再利用し、source adapter / typed evidence / risk estimator / decisionを追加・再編する。R0の独立RecognitionEngineは追加済み。[現APIと境界](recognition-api.md)。[R1 source adapter](recognition-source-evidence.md)を追加済み。risk estimatorは未実装。

2026-10-04 / P1実装。ユーザーの明示承認を受け、core/Sudachi/legacyをCargo workspaceへ分離。公開facadeは0.2開発版。

## 依存方向

応用層 → 公開facade → core。Sudachi等のbackendはcoreのMorphAnalyzer traitを実装する。coreはbackend固有型を公開せず、serde/serde_json/thiserrorだけへ依存する。Git・Markdown・Candle・Sudachi・sakuはcore依存に入れない。

- core: 原文、span、三軸score、issue、coverage、schema、trait、設定、評価結果の検証。
- Sudachi: owned辞書bytes/ローカルpathと設定、Mode A/B/C、全形態素、辞書/設定SHA256。モデル不要。
- facade: backendの組み立て、evaluate/evaluate_batch。標準はSudachi、SLMは任意。
- legacy: 既存Git/Markdown/RAG/要約/Candle runtimeと固有error。0.1動作を維持。

## 実装した互換性境界

公開facadeを0.2開発版へ変更し、defaultはSudachi、no-default-featuresはcoreのみ。旧KazeNhanhEngine/EngineConfig等はlegacy featureの再export、またはkaze_nhanh_legacy 0.1 crateを明示して使用する。mock_inferenceはlegacyを有効化し、暗黙runtime置換は引き続きしない。推論smoke/bench/旧workflowは必要featureを明示する。

これはdefault featureとAPI availabilityの破壊的変更なので、0.1の同版として黙って変更しない。publish/mergeは行わない。

## 段階境界

P1で検出精度を保証せず、未実装の判定はundetermined/null/全文未評価。P2でSudachi特徴を使う一次検出器、P3でSecondaryJudgeとrouting、有界worker、P4でScoreCalibratorと品質SLOを追加する。SLM APIのために空crateを先行生成しない。Sudachi経路へ要約のstopword除去を流用しない。

[0.2移行ガイドとfeature matrix](migration-0.2.md)を参照。workspaceはkaze_nhanh_core、kaze_nhanh_sudachi、kaze_nhanh_legacyとroot facadeで構成し、各通常依存の禁止条件をverify.ps1で検査する。

## P2追加

coreのPrimaryRules/DomainProfile/MorphologyFeaturesを標準Sudachi facadeへ注入。入力制約・文字/形態素反復等を検出し、意味軸は未評価のまま二次候補にする。SLM backendは持たず呼出0。評価fixture runnerは開発exampleへ分離し、通常runtimeにdatasetや学習依存を含めない。[一次検出仕様](primary-detection.md)。

P3制御契約: coreにSecondaryJudge/Factory/Workerを追加。任意Arc workerは1thread/有界queueで遅延loadし、共有呼出予算・deadline・厳格出力検証で保留する。新規モデル依存は追加しない。実CPUモデルadapterは別backendとして未実装。[詳細と同期backendの限界](secondary-judging.md)。

P3実験用adapter: optional qwen feature/crateでCPUの自然さpaired-label判定を追加。固定資産セットアップと独立token ID/CPU smokeは[Qwen手順](qwen-judge.md)を参照。実運用品質・意味adapter・CPU SLOは未受入。
