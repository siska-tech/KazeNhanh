# KazeNhanh 開発ロードマップ

取り組むべき次のタスク : 再設計計画 P2（一次検出MVP）→ P3（選択的SLM）

2026-10-04: [再設計監査・計画](../docs/KZN-REDESIGN-PLAN-001.md)を作成。[監査タスク](task-redesign-001-text-evaluation-audit.md)はcompleted、[移行タスク](task-redesign-002-text-evaluation-migration.md) / [Issue #2](https://github.com/siska-tech/KazeNhanh/issues/2)でP0完了。P1も完了。P2〜P5は未着手。以下の9件・44件は旧Git/要約機能の履歴であり、新基盤の進捗や検証済み品質を示しません。

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





P1進行中: 独立coreの8契約試験が成功。0.1互換を保った評価APIの追加を実装。Sudachi/legacy分離・0.2 API変更は自動承認レビューによる明示承認待ち。[API仕様002](../docs/KZN-API-SPEC-002.md)。

2026-10-04: ユーザーの明示承認後にcore/Sudachi/legacyをCargo workspaceへ分離し、0.2へ切替。P1完了。default/minimalの依存境界とモデル不要起動、owned辞書・Mode・原文span・並行評価、旧APIの回帰を検証。[0.2移行ガイド](../docs/migration-0.2.md)。以前の承認待ち記録は解消済み。
