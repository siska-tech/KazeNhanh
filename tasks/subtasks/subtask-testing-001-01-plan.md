---
status: completed
priority: medium
assignee: Backend
parent: task-testing-001-quality-assurance
start_date: 2025-11-07
end_date: 2025-11-07
tags: ["testing", "planning"]
depends_on: []
---

# サブタスク概要
テスト仕様書から必要なテスト種別とカバレッジ目標を分解し、実行順序と担当範囲を計画する。

## 完了条件
- テストスイートの一覧と優先度が整理されている。
- カバレッジ目標達成のための戦略が文書化されている。
- 必要なテストデータやモックの準備計画が立てられている。

## テストスイート一覧と優先順位
| スイートID | 対象領域                                                                                                                   | 優先度 | 目的/主な観点                                                            | カバレッジ指標                                     |
| ---------- | -------------------------------------------------------------------------------------------------------------------------- | ------ | ------------------------------------------------------------------------ | -------------------------------------------------- |
| UT-FOUND   | `src/foundation` (`git_service`, `markdown_service`, `nlp_service`)                                                        | P0     | ファウンデーション層の入出力正当性、エラー分岐網羅                       | C0≥95%, C1≥90%, クリティカルAPI境界値網羅          |
| UT-PIPE    | `src/pipeline` (`lex_rank_engine`, `hybrid_summarizer`, `git_native_rag`)                                                  | P0     | パイプライン構成要素のスコア計算・重み付けロジック検証                   | C0≥95%, C1≥90%, 主要分岐全網羅                     |
| UT-INF     | `src/inference` (`model`, `InferenceEngine`)                                                                               | P1     | モデル初期化・エラーハンドリング・リトライの検証                         | C0≥90%, C1≥85%                                     |
| IT-API     | `tests/integration/api_workflows.rs`（新規）                                                                               | P0     | 公開API（ラッパー経由）のシナリオ検証、Git → NLP → Summarizer の一気通貫 | 正常/異常シナリオ網羅、回帰ベクトルセット          |
| IT-RAG     | `tests/integration/git_native_rag.rs`（新規）                                                                              | P0     | `GitNativeRAG` パイプラインの差分収集と要約フロー検証                    | テストケース 8件以上、差分種別網羅                 |
| CONCUR     | `cargo test -- --test-threads=1` & `cargo test --release -- --ignored --nocapture` + `RUSTFLAGS="-Z sanitizer=thread"`想定 | P0     | マルチスレッド・`Mutex` ガードの競合検知、`Send + Sync`保証              | 10ケース以上の並列シナリオ、ThreadSanitizer検知0件 |
| PERF       | `cargo bench` + `criterion`ベンチ                                                                                          | P1     | LexRank、HybridSummarizer、RAGの性能基準測定                             | P95レイテンシ、スループット、メモリピーク記録      |
| SOAK       | `scripts/soak/`（新規）                                                                                                    | P2     | 3h連続実行時のリソースリーク検証                                         | CPU/メモリ指標収集、自動レポート                   |
| CI/CD      | `ci/github-actions/testing.yml`（新規 or 既存拡張）                                                                        | P0     | 上記スイートの自動化と成果物アーティファクト化                           | CI成功率100%、レポート添付                         |

## カバレッジ戦略
- `cargo llvm-cov` をベースに、foundation/pipeline レイヤを最優先とする日次カバレッジ計測をCIで可視化。
- クリティカル経路（Git差分→Markdown抽出→NLPトークン化→LexRank→ハイブリッド要約）の分岐網羅表を作成し、ユニットテストでカバレッジギャップをチェックリスト化。
- 例外経路（`KazeNhanhError` 各バリアント）をマトリクス化し、モック/スタブで強制的に発火させるテストを追加。
- ベンチマーク対象についてはベースライン計測値をスナップショットし、性能劣化を検知した場合に失敗させるしきい値を設定。

## テストデータ・モック準備計画
- Git差分用: `tests/fixtures/repos/*` にミニリポジトリ（変更種類:追加/削除/リネーム/バイナリ）を用意。`git_service` ヘルパで再利用できるよう `prepare_repository` をユニットテストが呼べるAPIに拡張。
- Markdown/NLP: `tests/fixtures/markdown_samples.yml` に多言語・コードブロック混在ケースを収集し、NLPモックで Sudachi 依存を回避できる軽量辞書ファイルを生成。
- 推論: candle モデルロードを差し替えるため、`InferenceEngine` に `ModelLoader` トレイトを導入し、モック実装 (`MockModelLoader`) でエラー注入。
- CI用: GitHub Actions でキャッシュ済みSudachi辞書・GGUFモデルをダウンロードするための事前スクリプト (`scripts/setup/download_assets.ps1`) を追加。

## 実行順序と担当範囲
1. 計画確定（本ドキュメント）→ foundation/pipeline ユニットテストの分担決定（Backend）
2. ユニットテスト整備完了後に結合テスト・並行性テストを統合ブランチで実施
3. ベンチ・ソークテストは検証用環境（self-hosted runner）を確保後に実施し、CIパイプラインへ段階的に組み込み
4. 各サブタスク完了時に `tasks/ROADMAP.md` を更新し、サマリーを README の開発進捗へ反映

## リスクと対応
- Sudachi辞書のライセンス・容量制約 → CI ではキャッシュ手法を採用、ローカルでは `make assets` コマンドで一括取得
- ThreadSanitizer 利用時の nightly 依存 → `rust-toolchain.toml` に nightly チャンネルを追加し、安定版との互換性検証を並行実施
- ベンチマークの環境差 → 目標値は相対比較 (±15%) とし、CI 失敗はトレンド変化検知のみで判断

