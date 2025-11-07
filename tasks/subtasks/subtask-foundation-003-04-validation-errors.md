---
status: completed
priority: medium
assignee: Backend
parent: task-foundation-003-git-service
start_date: 2025-11-07
end_date: 2025-11-07
tags: ["foundation", "git"]
depends_on: ["subtask-foundation-003-03-diff-collection"]
---

# サブタスク概要
入力検証やlibgit2エラーの扱いを整理し、`GitOperationError`などに正しくマッピングする。

## 完了条件
- 無効なパラメータやIO例外時に期待したエラー型が返る。
- エラーメッセージにパスや原因などの追加情報が含まれる。
- エラーパスをテストで再現し、ハンドリングを確認できる。

## 作業内容
- `GitReportOptions::validate` で拡張子入力・重複・カスタムプロンプトの検証を追加し、デフォルト拡張子の正規化を実装。
- `prepare_repository` のリポジトリオープン時エラーマッピングを詳細化し、`GitOperationError` 表示に原因メッセージを含めるよう更新。
- 破損コミットや不正引数を再現するユニットテストを追加し、期待するエラー種別とメッセージを検証。

## テスト
- `cargo test`

