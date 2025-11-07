---
status: completed
priority: high
assignee: Backend
start_date: 2025-11-07
end_date: 2025-11-07
tags: ["foundation", "git"]
depends_on: ["task-core-001-kaze-nhanh-engine"]
---

# タスク概要
git2-rsをラップした`GitService`を実装し、Markdown差分から追加行と行番号を抽出するFR4.1-4.2を満たす。

## 要件
- `Repository::open`から期間指定`revwalk`を構成し、`.md`/`.markdown`拡張子に絞ったdiffを取得する（`KZN-DETAIL-DESIGN-001` 4.1.3）。
- `diff.foreach`のコールバックで追加行のみを収集し、`FileDiff { path, added_lines }`を構築する（`KZN-DETAIL-DESIGN-001` 4.1.3、`KZN-REQ-SPEC-001` 3.4.2）。
- `GitReportOptions`の入力検証とエラーマッピング（`RepositoryNotFound`/`GitOperationError`）を実装する（`KZN-API-SPEC-001` 2.3, `KZN-DETAIL-DESIGN-001` 4.6）。

## サブタスク
- [x] [`subtask-foundation-003-01-spec-review`](subtasks/subtask-foundation-003-01-spec-review.md) Git差分要件と入力パラメータの整理
- [x] [`subtask-foundation-003-02-repo-access`](subtasks/subtask-foundation-003-02-repo-access.md) リポジトリオープンと期間フィルタの実装
- [x] [`subtask-foundation-003-03-diff-collection`](subtasks/subtask-foundation-003-03-diff-collection.md) 差分走査と`FileDiff`構築
- [x] [`subtask-foundation-003-04-validation-errors`](subtasks/subtask-foundation-003-04-validation-errors.md) 入力検証とエラーマッピング
- [x] [`subtask-foundation-003-05-tests-docs`](subtasks/subtask-foundation-003-05-tests-docs.md) テスト整備とドキュメント更新

## 実施内容
- `prepare_repository` で `Repository::open` と `revwalk` を初期化し、`days_since` に基づく期間フィルタ付き `RepositoryContext` を構築しました。
- `GitReportOptions::validate` と `effective_extensions` を実装し、入力検証とデフォルト拡張子適用を整理しました。
- Windows (MSVC) 環境での `git2` リンク問題に対応するため `build.rs` を整備し、テストが成功することを確認しました。
- `collect_markdown_diffs` を実装して Markdown 追加行を `FileDiff`/`LineAddition` に集約し、バイナリ差分・拡張子フィルタを含むユニットテストを追加しました。
- 削除・マージコミットを含むケースのテストを拡充し、README に GitService の使用例と制約・改善メモを反映しました。
- 入力パラメータ検証とエラーマッピングを強化し、欠損コミットや無効拡張子などの異常系テストを整備しました。

