---
status: completed
priority: low
assignee: Backend
parent: task-foundation-002-markdown-service
start_date: 2025-11-07
end_date: 2025-11-07
tags: ["documentation"]
depends_on: ["subtask-foundation-002-04-error-handling"]
---

# サブタスク概要
Markdownサービスの使用例・APIドキュメント・開発者向けガイドを整備し、利用時の注意点を共有する。

## 完了条件
- READMEや内部ドキュメントにサンプルコードが掲載されている。
- 行番号マッピングの制約や性能上の注意点が説明されている。
- 変更履歴や今後の改善点が整理されている。

## 実装メモ
- READMEの「開発進捗」に MarkdownService 系サブタスクの完了履歴を追記し、 foundation::markdown ガイドとサンプルコードを追加。
- 見出しタイトルの正規化と連続改行を含む範囲計算の制約について README で明記。
- 今後の改善点として、見出し以外のブロックセグメントを扱う可能性を ROADMAP の進捗記録に反映。

