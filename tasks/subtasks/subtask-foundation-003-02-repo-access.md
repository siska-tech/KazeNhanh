---
status: completed
priority: high
assignee: Backend
parent: task-foundation-003-git-service
start_date: 2025-11-07
end_date: 2025-11-07
tags: ["foundation", "git"]
depends_on: ["subtask-foundation-003-01-spec-review"]
---

# サブタスク概要
リポジトリオープンと期間フィルタリングを実装し、`revwalk`で対象コミットを抽出できるようにする。

## 実装メモ
- `prepare_repository` で `Repository::open`、`revwalk` 準備、期間フィルタリングをまとめた `RepositoryContext` を返すようにし、0コミットのケースでは `UnbornBranch` を考慮。
- 入力検証で `days_since` の下限チェックとパス空判定を追加し、`KazeNhanhError::InvalidInput` を返すようにした。
- Windows (MSVC) 用に `build.rs` で必要なシステムライブラリを明示的にリンクし、テスト環境でも `git2` が動作するように調整。

## 完了条件
- `Repository::open`と入力パスの検証が実装されている。
- `days_since`を元にした時間計算でコミットを適切にフィルタリングできる。
- エラー時に`KazeNhanhError::RepositoryNotFound`等が返ることを確認するテストがある。

