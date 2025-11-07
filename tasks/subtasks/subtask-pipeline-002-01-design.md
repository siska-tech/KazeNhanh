---
status: completed
priority: high
assignee: Backend
parent: task-pipeline-002-git-native-rag
start_date: 2025-11-07
end_date: 2025-11-07
tags: ["pipeline", "rag"]
depends_on: []
---

# サブタスク概要
GitNativeRAGの全工程を整理し、入力・出力・プロンプト形式および依存サービスのインターフェースを設計する。

## 完了条件
- 各ステップの前提条件と依存情報が明文化されている。
- プロンプトのテンプレート案と変数が定義されている。
- エラーや非対象ケースの処理方針がまとめられている。

## 進捗メモ
- `GitNativeRAG`実装に向けて、`git_service::collect_markdown_diffs`と`MarkdownService::map_document_structure`の入出力整理を完了。
- `InferenceEngine`経由のプロンプト合成パスとフォールバック条件を洗い出し、パイプライン工程のドラフトを策定。
- 実装確定版では `ContextualChange` + `ContextualAddition` に正規化し、`GitNativeRagReport` でフォールバックとサマリーフローを公開APIから扱えるよう整理。
- フォールバック条件 (`No diffs`/`InvalidInput`/`ModelInferenceError`) を `GitNativeRAG::execute` と `KazeNhanhEngine::run_git_native_rag` 双方でハンドリングする設計を決定し、ユーザーフレンドリーなメッセージ化を実装済み。

## 詳細設計メモ
- フロー分解
  1. `prepare_repository`→`collect_markdown_diffs`で`Vec<FileDiff>`を収集し、空の場合は即フォールバック。
  2. 差分対象ファイルのMarkdown本文をロードするために`MarkdownService`で構造解析し、`Vec<MarkdownSection>`を取得。
  3. `correlation_engine::correlate`で追加行とセクションを突合して`ContextualChange`にまとめ、`ContextualAddition`で行番号・内容を正規化しつつファイルごとにクラスタリング。
  4. `prompt_builder::build_prompt`でリポジトリ情報・クラスタ化された差分・空ケース理由をテンプレートへ埋め込み。
  5. `InferenceEngine::synthesize`へプロンプトを渡し、生成テキストを整形して最終レポートとして返却。
- データ構造のドラフト
  - 最終決定: `ContextualChange { file_path: String, section_title: Option<String>, section_level: Option<u8>, added_lines: Vec<ContextualAddition> }`
  - プロンプト入力は`repo_meta`（リポジトリ名・期間）と`change_clusters`を整形したMarkdownテキスト。
- エラー/フォールバック方針
  - 差分なし: `InvalidInput`ではなくユーザー向けメッセージ（`No relevant Markdown changes detected...`）を返却。
  - Markdown解析失敗: `KazeNhanhError::MarkdownParseError`をラップし、対象ファイル名を含むログを出力。
  - 推論失敗: `ModelInferenceError`や`InvalidInput`をフォールバックに変換しつつ、プロンプトと差分概要のトレースを残す（再試行方針は後続タスク）。

