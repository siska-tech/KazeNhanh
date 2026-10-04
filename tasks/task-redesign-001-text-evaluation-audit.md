---
status: completed
priority: high
assignee: Backend
start_date: 2026-10-04
end_date: 2026-10-04
tags: [audit, architecture, detection-first]
depends_on: []
---

# ローカルテキスト妥当性評価基盤への再設計監査・計画

## 成果物

[監査・移行計画](../docs/KZN-REDESIGN-PLAN-001.md)に、現在の実装根拠、再利用/分離方針、評価契約、選択的SLM、拡張点、P0〜P5の完了条件を記録。

## 完了範囲

監査と計画の作成のみ完了。基盤の実装・不具合修正・新モデルの導入は未着手。

## 検証

cargo fmt --checkとoffline metadata確認を実施。テスト対象の未登録を確認。offline testはSudachi依存未取得で開始できず、テスト成功や実モデル性能は主張しない。

## 次の実装

計画P0（テスト登録・fixture・本番/モック検証の分離）から開始し、P1（評価契約と責務分離）、P2（一次検出MVP）へ進む。
