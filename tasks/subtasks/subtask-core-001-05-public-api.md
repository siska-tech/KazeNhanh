---
status: completed
priority: medium
assignee: Backend
parent: task-core-001-kaze-nhanh-engine
start_date: 2025-11-07
end_date: 2025-11-07
tags: ["core", "api"]
depends_on: ["subtask-core-001-04-engine-init"]
---

# サブタスク概要
公開メソッドのシグネチャおよび内部パイプライン呼び出しのスタブを整備し、今後の実装がスムーズに行えるよう土台を作る。

## 完了条件
- `generate_git_report`など公開APIがコンパイル可能な形で定義されている。
- 内部で使用するパイプライン呼び出し／依存注入のトレイトや構造体が仮実装されている。
- TODOコメントや未実装箇所が明示され、後続タスクと整合している。

## 作業内容
- `KazeNhanhEngine` の公開メソッド群を実装し、Git差分収集・簡易要約・要約合成・重要文抽出・トークナイズをそれぞれ内部サービスに委譲。
- `generate_git_report` で `git_service` を利用したMarkdown差分の収集と簡易フォーマットを実装。
- `InferenceEngine` に簡易 `synthesize` を追加し、要約/合成APIから利用できるように調整。
- `TokenizedMorpheme` の再エクスポートと NLP サービスの公開化でトークン化結果をクライアントに返却可能にした。

## テスト
- `cargo test`

