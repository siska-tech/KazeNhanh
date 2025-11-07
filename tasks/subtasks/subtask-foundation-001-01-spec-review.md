---
status: completed
priority: high
assignee: Backend
parent: task-foundation-001-nlp-service
start_date: 2025-11-07
end_date: 2025-11-07
tags: ["foundation", "analysis"]
depends_on: []
---

# サブタスク概要
Sudachi設定ファイル仕様とNLP基盤要件を確認し、必要な初期化シーケンスと外部ファイル構成をまとめる。

## 完了条件
- 設計書該当部分の要約メモが作成されている。
- 必要な環境変数や設定パラメータが列挙されている。
- 今後の実装で参照するテストデータの方針が決まっている。

## レビュー結果サマリ
- `EngineConfig` から渡される `dictionary_bytes` / `settings_bytes` を `'static [u8]` のまま扱い、`include_bytes!` でパッケージングする前提が API / 詳細設計の双方で固定（KZN-API-SPEC-001 §0.2, §2.1・KZN-DETAIL-DESIGN-001 §4.4.1）。
- `NlpService::new` は `settings_bytes` を UTF-8 として取り込み `Config::from_str` で Sudachi 設定を復元し、`system_dict` に `dictionary_bytes` を差し込んだ後 `Tokenizer::new` を生成するフロー（KZN-DETAIL-DESIGN-001 §4.1.1）。
- Sudachi のエラーは L1 で `KazeNhanhError::DictionaryLoadError`、L2 で `TokenizationError` にマッピングする規約が定義済み（KZN-DETAIL-DESIGN-001 §4.1.1, §4.4 / KZN-API-SPEC-001 §0.3）。
- 文分割は `saku` を用いた `split_sentences` ヘルパーで提供し、L1 からは `kaze_nhanh::foundation::nlp::split_sentences` として直接利用する（KZN-DETAIL-DESIGN-001 §4.1.1, KZN-ARC-DESIGN-001 §105）。
- Sudachi / saku の採用理由と純 Rust 方針はアーキテクチャ指針（REQ 2.2, REQ 5.0）に明記され、辞書ロードはファイルパスへ依存しないゼロセットアップが必須（KZN-ARC-DESIGN-001 §18, §103）。

## 追加で確認が必要な事項
- `settings_bytes` に含める Sudachi JSON のテンプレート値（辞書パス、形態素分割モードなど）を docs 配下で特定し、リリース artefact の配置パスと整合させる。
- `user_dictionary` を利用しない前提だが、Option を将来的に開くかどうか要判断。必要な場合は `EngineConfig` 拡張と合わせて依存タスクへ波及。
- Sudachi / saku のテストデータとしてどの文章セットを用いるか決める必要がある（例: 2,000字程度の技術文書、口語文）。CI 環境で扱えるサイズに制約がないか要調査。

## 次アクション
- `subtask-foundation-001-02-inmemory-init` で `NlpService::new` の実装へ着手し、設定 JSON のスキーマと `Tokenizer::new` 呼び出しをコード化する。
- Sudachi / saku アセットの配置パスとダウンロード元を整理し、`README` あるいは `docs` の補足に追記するドラフトを準備する。
- テスト用に想定する日本語サンプル文書を 2 種類以上ピックアップし、`subtask-foundation-001-05-tests-docs` へ引き継ぐ。

