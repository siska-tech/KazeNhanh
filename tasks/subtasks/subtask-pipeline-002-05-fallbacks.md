---
status: completed
priority: medium
assignee: Backend
parent: task-pipeline-002-git-native-rag
start_date: 2025-11-07
end_date: 2025-11-07
tags: ["pipeline", "rag"]
depends_on: ["subtask-pipeline-002-04-prompt"]
---

# サブタスク概要
対象差分が存在しない場合や入力不正ケースに備え、フォールバックメッセージとエラー処理を実装する。

## 完了条件
- 差分なし・解析失敗時にユーザーフレンドリーなメッセージが返る。
- `InvalidInput`などのエラーが適切にハンドリングされている。
- 対応するテストが追加され、期待結果が検証されている。

## 進捗メモ
- `GitNativeRAG::execute` で推論エラー(`InvalidInput`/`ModelInferenceError`)を検知し、詳細付きフォールバックレポートを返すガードを追加。
- `KazeNhanhEngine::run_git_native_rag` が `InvalidInput` をユーザーフレンドリーなメッセージに変換するようにし、リポジトリ情報を保持したまま返送。
- `prompt_builder` と `correlation_engine` のコンテキスト構造を拡張し、フォールバックメッセージ整形の基礎データを統一。

