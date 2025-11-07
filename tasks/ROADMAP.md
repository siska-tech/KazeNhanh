# KazeNhanh 開発ロードマップ

取り組むべき次のタスク : （全タスク完了）
完了タスク件数 / 総タスク件数 : 8 / 8
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

> 各タスクは設計書の要件を満たす実装および検証を対象とし、進行状況に応じてステータスと日付を更新すること。

