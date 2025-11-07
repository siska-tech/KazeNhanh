---
status: completed
priority: medium
assignee: Backend
parent: task-foundation-001-nlp-service
start_date: 2025-11-07
end_date: 2025-11-07
tags: ["foundation", "testing"]
depends_on: ["subtask-foundation-001-03-tokenize", "subtask-foundation-001-04-split-sentences"]
---

# サブタスク概要
NLPサービスのテストケースとドキュメントを整備し、エラーハンドリングや使用例を明記する。

## 完了条件
- 単体テストが主要パスとエラーパスをカバーし、CIで実行される。
- READMEまたは内部ドキュメントに利用例が追記されている。
- 既知の制約や今後の課題がメモとして残されている。


## 実施内容
- `split_sentences` 周りの単体テストを拡充し、ASCII句読点・日本語引用符・連続終端記号と空白のケースをカバーしました。
- Sudachi辞書を外部同梱しない前提での `foundation::nlp` 利用例と既知の制約メモを README に追記しました。
- 文末終端後の句読点・閉じ括弧を正しく連結するよう `split_sentences` のヒューリスティックを更新しました。
