---
status: completed
priority: high
assignee: Backend
parent: task-foundation-003-git-service
start_date: 2025-11-07
end_date: 2025-11-07
tags: ["foundation", "git"]
depends_on: ["subtask-foundation-003-02-repo-access"]
---

# サブタスク概要
`diff.foreach`コールバックを用いてMarkdownファイルの追加行を収集し、`FileDiff`構造体のリストを生成する。

## 完了条件
- 追加行のみを抽出し、行番号とテキストが正しく格納される。
- 拡張子フィルタリングとバイナリ差分スキップが動作する。
- 複数コミットに跨る変更をテストで検証している。

## 作業内容
- `collect_markdown_diffs` を実装し、Markdown拡張子の差分から追加行のみを `FileDiff`/`LineAddition` として集計。
- バイナリ差分・非対象拡張子のスキップロジックと行番号の正規化処理を追加。
- マルチコミットとバイナリ差分パスを網羅するユニットテストを整備し、`cargo test` で検証。

