# KZN-ARC-DESIGN-002: 評価責務の分離

2026-10-04 / P1設計。全体workspace分離は自動承認レビューで停止し、具体的なcrate分離・0.2 API変更への承認待ち。

## 依存方向

応用層 → 公開facade → core。Sudachi等のbackendはcoreのMorphAnalyzer traitを実装する。coreはbackend固有型を公開せず、serde/serde_json/thiserrorだけへ依存する。Git・Markdown・Candle・Sudachi・sakuはcore依存に入れない。

- core: 原文、span、三軸score、issue、coverage、schema、trait、設定、評価結果の検証。
- Sudachi: owned辞書bytes/ローカルpathと設定、Mode A/B/C、全形態素、辞書/設定SHA256。モデル不要。
- facade: backendの組み立て、evaluate/evaluate_batch。標準はSudachi、SLMは任意。
- legacy: 既存Git/Markdown/RAG/要約/Candle runtimeと固有error。0.1動作を維持。

## 互換性の具体案（承認待ち）

公開facadeを0.2開発版へ変更し、defaultはSudachi、no-default-featuresはcoreのみ。旧KazeNhanhEngine/EngineConfig等はlegacy featureの再export、またはkaze_nhanh_legacy 0.1 crateを明示して使用する。mock_inferenceはlegacyを有効化し、暗黙runtime置換は引き続きしない。推論smoke/bench/旧workflowは必要featureを明示する。

これはdefault featureとAPI availabilityの破壊的変更なので、0.1の同版として黙って変更しない。publish/mergeは行わない。

## 段階境界

P1で検出精度を保証せず、未実装の判定はundetermined/null/全文未評価。P2でSudachi特徴を使う一次検出器、P3でSecondaryJudgeとrouting、有界worker、P4でScoreCalibratorと品質SLOを追加する。SLM APIのために空crateを先行生成しない。Sudachi経路へ要約のstopword除去を流用しない。
