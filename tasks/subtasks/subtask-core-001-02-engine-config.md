---
status: completed
priority: high
assignee: Backend
parent: task-core-001-kaze-nhanh-engine
start_date: 2025-11-07
end_date: 2025-11-07
tags: ["core", "api"]
depends_on: ["subtask-core-001-01-spec-review"]
---

# サブタスク概要
`EngineConfig`構造体とコンストラクタヘルパーを実装し、静的バイトスライスから必要リソースを受け取れるようにする。

## 完了条件
- `EngineConfig`フィールドと`new`ヘルパーが設計書通りに実装されている。
- 静的リソースの型が`&'static [u8]`で統一され、テストまたはコンパイルで確認できる。
- ドキュメントコメントに使用方法のサンプルが追加されている。

