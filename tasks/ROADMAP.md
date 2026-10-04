# KazeNhanh 開発ロードマップ

取り組むべき次のタスク : recognition risk再設計 R1（OCR/ASR evidence）→ R2（軽量risk baseline）

2026-10-04: [再設計案002](../docs/KZN-REDESIGN-PLAN-002.md)を作成。OCR/ASR共通recognition riskを主目的に変更し、naturalnessは補助、Qwen judgeは実験比較用とする。R0契約/保留は実装済み、R1〜R4は未着手。誤認識検出品質の改善を意味しない。以下のP0〜P3は従来計画での実績・履歴として保持。

2026-10-04: [再設計監査・計画](../docs/KZN-REDESIGN-PLAN-001.md)を作成。[監査タスク](task-redesign-001-text-evaluation-audit.md)はcompleted、[移行タスク](task-redesign-002-text-evaluation-migration.md) / [Issue #2](https://github.com/siska-tech/KazeNhanh/issues/2)でP0完了。P1/P2も完了。P3は制御契約を実装して進行中、P4/P5は未着手。以下の9件・44件は旧Git/要約機能の履歴であり、新基盤の進捗や検証済み品質を示しません。

P0完了: 明示fake注入、実辞書・合成/学習済みGGUFのCPU推論、公式tokenizerの独立参照ID照合を検証。固定モデルセットアップと実モデルCIを追加。Windows/Linux QA・TSan・Criterion/Soakもremote成功。

## 旧Git・要約機能のタスク記録
完了タスク件数 / 総タスク件数 : 9 / 9
完了サブタスク件数 / 総サブタスク件数 : 44 / 44 （進捗率: 100%）

| No  | タスクID                             | 概要                                 | ステータス | 優先度 | サブタスク                                            | 依存関係             | 参照ドキュメント                          |
| --- | ------------------------------------ | ------------------------------------ | ---------- | ------ | ----------------------------------------------------- | -------------------- | ----------------------------------------- |
| 1   | task-core-001-kaze-nhanh-engine      | ファサードエンジンとエラー統合を実装 | completed  | high   | [一覧](task-core-001-kaze-nhanh-engine.md) (6/6)      | なし                 | KZN-API-SPEC-001, KZN-DETAIL-DESIGN-001   |
| 2   | task-foundation-001-nlp-service      | SudachiベースNLPサービス             | completed  | high   | [一覧](task-foundation-001-nlp-service.md) (5/5)      | task-core-001        | KZN-DETAIL-DESIGN-001, KZN-REQ-SPEC-001   |
| 3   | task-foundation-002-markdown-service | Markdown構造マッピング               | completed  | medium | [一覧](task-foundation-002-markdown-service.md) (5/5) | task-core-001        | KZN-DETAIL-DESIGN-001, KZN-REQ-SPEC-001   |
| 4   | task-foundation-003-git-service      | Git差分抽出サービス                  | completed  | high   | [一覧](task-foundation-003-git-service.md) (5/5)      | task-core-001        | KZN-DETAIL-DESIGN-001, KZN-REQ-SPEC-001   |
| 5   | task-inference-001-inference-engine  | candle推論エンジン実装               | completed  | high   | [一覧](task-inference-001-inference-engine.md) (6/6)  | task-core-001        | KZN-DETAIL-DESIGN-001, KZN-ARC-DESIGN-001 |
| 6   | task-pipeline-001-hybrid-summarizer  | HybridSummarizerとLexRank            | completed  | medium | [一覧](task-pipeline-001-hybrid-summarizer.md) (5/5)  | foundation/inference | KZN-DETAIL-DESIGN-001                     |
| 7   | task-pipeline-002-git-native-rag     | GitNativeRAGパイプライン             | completed  | high   | [一覧](task-pipeline-002-git-native-rag.md) (6/6)     | foundation/inference | KZN-DETAIL-DESIGN-001, KZN-REQ-SPEC-001   |
| 8   | task-testing-001-quality-assurance   | 総合テスト体制整備                   | completed  | medium | [一覧](task-testing-001-quality-assurance.md) (6/6)   | パイプライン完了後   | KZN-TEST-SPEC-001                         |
| 9   | task-demo-001-git-sample-repo        | デモ用Gitリポジトリ整備               | completed  | medium | [一覧](task-demo-001-git-sample-repo.md) (0/0)        | なし                 | README, demo/repo                        |

> 各タスクは設計書の要件を満たす実装および検証を対象とし、進行状況に応じてステータスと日付を更新すること。





P1完了: 独立coreの契約試験、新Sudachi・legacyの分離、0.2 feature境界を検証。[API仕様002](../docs/KZN-API-SPEC-002.md)。

2026-10-04: ユーザーの明示承認後にcore/Sudachi/legacyをCargo workspaceへ分離し、0.2へ切替。P1完了。default/minimalの依存境界とモデル不要起動、owned辞書・Mode・原文span・並行評価、旧APIの回帰を検証。[0.2移行ガイド](../docs/migration-0.2.md)。以前の承認待ち記録は解消済み。

2026-10-04: P2一次検出MVPを実装。profile・形態素features・説明可能rules、4用途共通APIとJSON runner。34 fixture全期待一致、正常19件の誤警報0、意味保留4件、SLM呼出0。[検出範囲](../docs/primary-detection.md)。

2026-10-04: P3の選択的二次worker・遅延load・共有予算・deadline・有界queue・厳格出力と保留を実装。優先はOCR/ASR。実モデル日本語judge/tokenizer bundle/CPU SLOは未完了。[制御契約](../docs/secondary-judging.md)。

2026-10-04: P3実験用Qwen自然さadapter・固定資産setup・独立token ID照合・CPU smokeを追加。P3全体の品質/意味adapter/SLO受入は未完了。[adapter](../docs/qwen-judge.md)。

2026-10-04: 提供PP-OCRv6 medium実認識5件を保存・投入。画像転記はユーザー確認済み。不一致3件すべてを現gate/実験用Qwenが検出できず、P3品質未受入を維持。[実測](../docs/ocr-samples-user-001.md)。

2026-10-04: 追加OCR 2画像/10件を原文・confidence・document ID付きで保存（計3画像/15件）。「体系キープ」「10kgやせる」の転記はユーザー確認済み、他8件は画像転記未確認。今回の転記差5件（確認済み1件）すべてが一次gateを通過。runner既定はモデル不要、SLM呼出/forward=0、Qwen比較は明示opt-inへ変更。P3品質未受入を維持。[追加データと観察](../docs/ocr-samples-user-002-003.md)。

2026-10-04: [R0 API](../docs/recognition-api.md)を追加。別schema・risk/null・evidence欠測・review/undetermined、モデル不要facade/CLI、提供OCR15件の保持/保留を検証。低リスク受理とrisk estimatorは未実装。
