---
status: completed
priority: medium
assignee: Backend
parent: task-foundation-001-nlp-service
start_date: 2025-11-07
end_date: 2025-11-07
tags: ["foundation", "nlp"]
depends_on: ["subtask-foundation-001-02-inmemory-init"]
---

# サブタスク概要
文分割関数`split_sentences`をラップし、`kaze_nhanh::foundation::nlp`モジュールから再エクスポートできるよう統合する。

## 完了条件
- sakuクレートを利用した文分割が実装されている。
- FFIやローカル依存がなく、ユニットテストで主要ケースを検証している。
- パブリックAPIから利用するためのモジュール構造が整備されている。

## 実施内容
- `saku::SentenceTokenizer` と補助ヒューリスティックを用いて `split_sentences` を実装し、日本語句読点・ASCII句読点・改行を含む文境界を正規化しました。
- `foundation::nlp` モジュールから `split_sentences` を再エクスポートし、外部API (`kaze_nhanh::foundation::nlp`) からの利用経路を整備しました。
- ユニットテストを追加し、空入力、改行、複数の疑問符・感嘆符を含むケースをカバーしました。

