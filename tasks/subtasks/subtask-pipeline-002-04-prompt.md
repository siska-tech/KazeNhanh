---
status: completed
priority: medium
assignee: Backend
parent: task-pipeline-002-git-native-rag
start_date: 2025-11-07
end_date: 2025-11-07
tags: ["pipeline", "rag"]
depends_on: ["subtask-pipeline-002-03-correlation"]
---

# サブタスク概要
RAG用プロンプトの生成ロジックを実装し、`InferenceEngine`への呼び出しと結果加工を行う。

## 完了条件
- 変更リストから定義済みフォーマットのプロンプト文字列が生成される。
- 推論呼び出し時のエラーハンドリングやリトライ方針が決まっている。
- 出力整形と返却値が仕様通りになっている。

## 進捗メモ
- `pipeline/prompt_builder.rs` を追加し、ファイル・セクション単位で整形したRAGプロンプト生成ロジックとフォールバックメッセージを実装。
- `GitNativeRAG::execute` を実装して差分取り込み→相関→プロンプト組み立て→推論呼び出しのパイプラインを統合。
- 推論成功/差分なし双方のユニットテストを整備し、`KazeNhanhEngine::run_git_native_rag` 経由でAPI公開。

