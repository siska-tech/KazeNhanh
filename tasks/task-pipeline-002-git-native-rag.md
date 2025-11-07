---
status: completed
priority: high
assignee: Backend
start_date: 2025-11-07
end_date: 2025-11-07
tags: ["pipeline", "rag"]
depends_on: ["task-foundation-002-markdown-service", "task-foundation-003-git-service", "task-inference-001-inference-engine"]
---

# タスク概要
GitNativeRAGパイプラインと`correlation_engine`を実装し、FR4の差分解析からレポート生成までを完結させる。

## 要件
- `GitNativeRAG::execute`でGit差分収集、Markdown構造解析、相関、合成プロンプト生成を順に処理する（`KZN-DETAIL-DESIGN-001` 4.3.3）。
- `correlation_engine::correlate`で`FileDiff`と`MarkdownSection`を突合し、`ContextualChange`リストを構築する（`KZN-DETAIL-DESIGN-001` 4.3.4、`KZN-REQ-SPEC-001` 3.4.4）。
- 処理対象がない場合のメッセージや`InvalidInput`エラー処理など、設計書に示されたフォールバックを実装する（`KZN-DETAIL-DESIGN-001` 4.3.3, 4.6）。

## 進捗メモ
- 既存の`git_service`・`markdown_service`・`hybrid_summarizer`モジュールを調査し、GitNativeRAGが再利用する差分収集・Markdown構造抽出・推論呼び出しのAPIを整理。
- `correlation_engine`の`ContextualChange`設計とプロンプト生成フローの要件を確認し、実装順序（差分取得→Markdown解析→相関→プロンプト出力）をドラフト。
- `GitNativeRAG::ingest_diffs`を実装し、差分抽出とMarkdown構造解析を統合するエントリポイントとテストスイートを整備。
- `correlation_engine::correlate`で`DiffIngestionEntry`をセクション単位にクラスタリングする処理を実装し、セクション未検出時のフォールバックと代表的パターンのユニットテストを追加。
- RAGプロンプト生成モジュールと`GitNativeRAG::execute`を追加し、推論呼び出し・フォールバック処理・`KazeNhanhEngine::run_git_native_rag`の公開APIを構築。
- 推論失敗/入力不正時のフォールバックメッセージを整備し、`GitNativeRAG`およびエンジンファサードでユーザーフレンドリーにリカバリーする経路を実装。
- GitNativeRAGの利用手順を `docs/git_native_rag.md` に整理し、ユニットテスト (`cargo test`) で差分無し/推論成功シナリオをカバーしてシナリオ整合性を検証。
- 全サブタスクとドキュメント更新を完了し、README/ROADMAP へ進捗反映、`cargo test` にて最終回帰確認済み。

## サブタスク
- [x] [`subtask-pipeline-002-01-design`](subtasks/subtask-pipeline-002-01-design.md) RAGパイプライン工程とプロンプト要件整理
- [x] [`subtask-pipeline-002-02-diff-ingest`](subtasks/subtask-pipeline-002-02-diff-ingest.md) Git差分とMarkdown構造の統合処理実装
- [x] [`subtask-pipeline-002-03-correlation`](subtasks/subtask-pipeline-002-03-correlation.md) `correlation_engine`実装と検証
- [x] [`subtask-pipeline-002-04-prompt`](subtasks/subtask-pipeline-002-04-prompt.md) プロンプト生成と推論呼び出し整理
- [x] [`subtask-pipeline-002-05-fallbacks`](subtasks/subtask-pipeline-002-05-fallbacks.md) フォールバック処理とエラーハンドリング
- [x] [`subtask-pipeline-002-06-tests-docs`](subtasks/subtask-pipeline-002-06-tests-docs.md) テストデータ整備とドキュメント更新

