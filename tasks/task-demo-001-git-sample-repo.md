---
status: completed
priority: medium
assignee: Backend
start_date: 2025-11-08
end_date: 2025-11-08
tags: ["demo", "git"]
depends_on: []
---

# タスク概要
KazeNhanh向けのデモ用ローカルGitリポジトリを整備し、Tauriシミュレータやドキュメントで即座に利用できるようにする。

## 要件
- `demo/repo` 配下に実プロジェクトを想定したフォルダ構成とサンプルコード・ドキュメントを配置する。
- 15〜20件程度のコミット履歴と複数ブランチを持つGitリポジトリを初期化し、差分解析や検索のデモに適した履歴を用意する。
- タグやリリースポイント、コンフリクトシナリオを含め、KazeNhanhのGit解析機能を試せるシナリオガイドを同梱する。
- READMEおよびタスク、ロードマップへ進捗を反映する。

## 進捗メモ
- 2025-11-08: `demo/repo` に WindLog Analyzer サンプルリポジトリを構築。17件のメインブランチコミットと3本の派生ブランチ（`feature/rich-logging`, `feature/config-refresh`, `bugfix/parse-empty`）を作成。




