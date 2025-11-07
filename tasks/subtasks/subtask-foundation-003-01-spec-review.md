---
status: completed
priority: high
assignee: Backend
parent: task-foundation-003-git-service
start_date: 2025-11-07
end_date: 2025-11-07
tags: ["foundation", "git"]
depends_on: []
---

# サブタスク概要
Git差分処理の要件と`GitReportOptions`仕様を整理し、対象期間や拡張子フィルタリングのルールを明確にする。

## 完了条件
- 設計書の該当範囲を要約したメモが用意されている。
- days_since・target_extensionsなどの入力条件が整理されている。
- 想定されるGit履歴構成（単体コミット、マージコミット等）が把握されている。

## レビュー結果サマリ
- `GitReportOptions` は `repo_path` / `days_since` / `target_extensions` / `custom_prompt` を保持し、拡張子は未指定時に `.md` と `.markdown` をデフォルト適用する設計（KZN-API-SPEC-001 §2.3）。`days_since` が 0 以下の入力は `InvalidInput` としてハンドリングする方針が明記されている。
- `get_markdown_diffs` は `Repository::open` 後に `revwalk` で期間内コミットを走査し、`diff_tree_to_tree` と `DiffOptions::pathspec` で Markdown のみを対象に抽出。`diff.foreach` で追加行だけを `FileDiff { path, added_lines }` に蓄積する仕様（KZN-DETAIL-DESIGN-001 §4.1.3）。
- Git 履歴から得た差分は L2 パイプライン `GitNativeRAG` の入力になり、Markdown セクション分割結果と突き合わせてレポートを生成する（KZN-DETAIL-DESIGN-001 §4.1.3, §4.4.2）。
- エラーマッピングは `RepositoryNotFound`（リポジトリ未検出）、`GitOperationError`（git2-rs エラー）、`DiffParseError`（差分解析失敗）、`MarkdownParseError` 等を `KazeNhanhError` として伝搬させる設計（KZN-API-SPEC-001 §2.3, 表2.2 / KZN-DETAIL-DESIGN-001 §4.1.3）。
- `custom_prompt` が `None` の場合はテンプレートプロンプトを使用すること、`target_extensions` が `None` の場合は `.md` / `.markdown` を採用することが API と要件仕様の双方に記載されている（KZN-API-SPEC-001 §2.3, KZN-REQ-SPEC-001 §3.4）。

## 追加で確認が必要な事項
- マージコミット（親が複数）の扱いは `parent(0)` のみを比較する実装案になっているため、必要に応じて全親を巡回するかどうか判断が必要。
- `revwalk` の停止条件はコミットタイムスタンプのみで、タイムゾーン差異や author/committer の時刻ズレが許容範囲かを検証する。
- `custom_prompt` のテンプレート内容とローカライズ要件が未確定。README に載せる使用例と整合させる必要がある。

## 次アクション
- `subtask-foundation-003-02-repo-access` で `Repository::open` と入力バリデーション（`days_since` の下限チェック等）を具体化する。
- `subtask-foundation-003-03-diff-collection` で `FileDiff` 生成ロジックとマージコミット対応の可否を検証し、テストケースを整備する。
- `subtask-foundation-003-05-tests-docs` にて `GitReportOptions` サンプルとエラーケースを README / docs に反映する案を検討する。

