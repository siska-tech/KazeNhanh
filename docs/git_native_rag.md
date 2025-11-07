# GitNativeRAG パイプライン設計書

GitNativeRAG は FR4「Git Native RAG レポート」シナリオを完結させるためのパイプラインです。本設計書では、アーキテクチャ、データ構造、処理フロー、フォールバック戦略、公開 API、テスト観点を整理します。

## 1. アーキテクチャ概要

- **Facade 層**: `KazeNhanhEngine::run_git_native_rag` が外部向け API。内部で `GitNativeRAG::execute` を呼び出し、成功時は要約、失敗時はフォールバックメッセージを返却します。
- **主要モジュール**
  - `foundation::git_service` … 差分収集 (`prepare_repository`, `collect_markdown_diffs`).
  - `foundation::markdown_service` … Markdown 構造解析 (`MarkdownService::map_document_structure`).
  - `pipeline::correlation_engine` … 追加行と見出しの突合 (`correlate`).
  - `pipeline::prompt_builder` … プロンプト文字列整形 (`build_prompt`, `fallback_message`).
  - `inference::InferenceEngine` … Candle ベースの要約生成。
- **成果物**: `GitNativeRagReport { repo_path, contextual_changes, content }` を返却。`content` は `GitNativeRagContent::Synthesized { prompt, summary }` または `::Fallback { message }`。

## 2. データ構造

```text
DiffIngestionResult {
  repo_path: String,
  entries: Vec<DiffIngestionEntry>
}

DiffIngestionEntry {
  file_path: String,
  added_lines: Vec<git_service::LineAddition>,
  sections: Vec<MarkdownSection>,
  document: String
}

ContextualChange {
  file_path: String,
  section_title: Option<String>,
  section_level: Option<u8>,
  added_lines: Vec<ContextualAddition>
}

ContextualAddition {
  line_number: u32,
  content: String
}

GitNativeRagReport {
  repo_path: String,
  contextual_changes: Vec<ContextualChange>,
  content: GitNativeRagContent
}
```

- `ContextualAddition` はプロンプト整形や外部公開に適した最小情報のみを保持するラッパー。
- `GitNativeRagContent` は成功／失敗の両経路を統合し、呼び出し側の分岐処理を簡素化します。

## 3. 処理フロー

1. **差分準備** (`GitNativeRAG::ingest_diffs`)
   - `prepare_repository` で `RepositoryContext` を構築。コミットが存在しない・ブランチ未初期化などの場合は空結果を返却。
   - `collect_markdown_diffs` で Markdown 追加行を収集し、ファイル本体を読み込んで `DiffIngestionEntry` を作成。
2. **相関付け** (`correlation_engine::correlate`)
   - 見出し開始行・タイトル・レベルをキーに `LineAddition` をグルーピング。
   - 見出しが見つからない行は `(no heading)` セクションとしてクラスタリング。
3. **プロンプト生成** (`prompt_builder::build_prompt`)
   - リポジトリ情報・期間・任意カスタムプロンプトをヘッダに整形。
   - ファイル単位に「セクション + 追加行」を Markdown 風に列挙し、末尾改行はトリミング。
4. **推論** (`InferenceEngine::synthesize`)
   - プロンプトが空白の場合は `InvalidInput`。
   - `ModelInferenceError` など推論失敗時はフォールバックに変換しつつ、失敗メッセージを `GitNativeRagReport` に格納。
5. **レポート構築**
   - 成功時は `GitNativeRagReport::synthesized` を返し、フォールバック時は `GitNativeRagReport::fallback` でメッセージを提供。

## 4. プロンプト設計

```
You are an AI assistant that reviews Git markdown changes and produces actionable insights.
Repository: {repo_path}
Time Window: Last {days_since} day(s)

Additional Analyst Guidance:
{custom_prompt}

Summarize the following contextualized changes.

File: {file_path}
  Section: {section_title or (no heading)} (level {section_level or -})
    +{line_number} {content}
```

- `custom_prompt` が空の場合はセクション自体を出力しない。
- 追加行は `trim()` 済みで、要約エンジンに過剰な空白を渡さない。

## 5. フォールバック戦略

| ケース   | トリガ                             | 処理                                    | 返却メッセージ例                               |
| -------- | ---------------------------------- | --------------------------------------- | ---------------------------------------------- |
| 差分なし | `DiffIngestionResult.entries` が空 | 即 `fallback`                           | `No relevant Markdown additions detected ...`  |
| 無効入力 | 空プロンプト / API 入力不備        | `run_git_native_rag` でフォールバック化 | `GitNativeRAG request was invalid ...`         |
| 推論失敗 | `ModelInferenceError`              | フォールバックへ変換                    | `GitNativeRAG could not produce a summary ...` |
| その他   | I/O 等の致命エラー                 | 呼び出し元へ伝播                        | -                                              |

## 6. 公開 API 利用例

```rust,ignore
use kaze_nhanh::{GitReportOptions, KazeNhanhEngine, EngineConfig};

const MODEL_BYTES: &[u8] = include_bytes!("../resources/llama-mock.gguf");
const DICT_BYTES: &[u8] = include_bytes!("../resources/system.dic");
const SETTINGS_BYTES: &[u8] = include_bytes!("../resources/sudachi.json");

fn run_git_native_rag() -> Result<(), kaze_nhanh::KazeNhanhError> {
    // NOTE: include_bytes! のパスは実環境のアセットに置き換えてください。
    let config = EngineConfig::new(MODEL_BYTES, DICT_BYTES, SETTINGS_BYTES);
    let engine = KazeNhanhEngine::new(config)?;

    let options = GitReportOptions {
        repo_path: "./docs",
        days_since: 3,
        target_extensions: None,
        custom_prompt: Some("Highlight customer-facing impact."),
    };

    let report = engine.run_git_native_rag(&options)?;

    match report.content {
        kaze_nhanh::GitNativeRagContent::Synthesized { summary, .. } => {
            println!("Generated summary:\n{}", summary);
        }
        kaze_nhanh::GitNativeRagContent::Fallback { message } => {
            println!("Fallback:\n{}", message);
        }
    }

    Ok(())
}
```

## 7. テスト・ドキュメンテーション

- `cargo test` により以下をカバー：
  - `pipeline::git_native_rag::tests` … 差分なし／差分あり／推論失敗経路。
  - `pipeline::correlation_engine::tests` … 見出し突合ロジック。
  - `pipeline::prompt_builder::tests` … カスタムプロンプトや `(no heading)` 処理。
- README / ROADMAP に進捗を反映済み。
- 品質保証フェーズ (`task-testing-001-quality-assurance`) で統合テスト・性能計測を拡張予定。

## 8. 今後の展望

- プロンプト構造を JSON/Markdown 以外の形式に切り替えるオプションの検討。
- エラー発生時に差分概要とプロンプトを `tracing` へ出力し、運用時の可観測性を向上。
- 大規模リポジトリ向けにストリーミング処理やページングを追加し、差分量に応じた性能最適化を行う。

以上により、GitNativeRAG パイプラインの実装・設計・ドキュメント整備まで完了しています。後続の QA タスクでシナリオテストと性能検証を実施し、FR4 要件の最終確認を行います。

