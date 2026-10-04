---
status: progress
priority: high
assignee: Backend
start_date: 2026-10-04
end_date:
tags: [redesign, testing, setup, sudachi]
depends_on: [task-redesign-001-text-evaluation-audit]
issue: https://github.com/siska-tech/KazeNhanh/issues/2
branch: feat/2-text-evaluation-foundation
---

# テキスト妥当性評価基盤への移行

[Issue #2](https://github.com/siska-tech/KazeNhanh/issues/2) / [設計・完了条件](../docs/KZN-REDESIGN-PLAN-001.md)

## 進捗

- [ ] P0: 検証基盤復旧
- [ ] P1: 評価契約・責務分離
- [ ] P2: 一次検出MVP
- [ ] P3: 選択的SLM
- [ ] P4: 校正・品質受入
- [ ] P5: 拡張・移行リリース

## 初回着手範囲

新PC向けセットアップ、固定バージョンのSudachi辞書取得、結合・並行性テスト登録、本番buildとモック推論テストの区別、実辞書smokeを整備する。

P0全体の完了には実モデル資産でのtokenizer照合、fake注入と本番runtimeの分離の完了、実モデル試験も必要。今回のモック試験で実SLMの品質・速度を保証しない。

## 2026-10-04 初回作業結果

- [x] Issue起票・作業ブランチ作成
- [x] Cargo PATH外の検出、MSVC検出、locked依存取得、固定Sudachi辞書のSHA256検証とlicense保存
- [x] 結合3件・並行性2件を明示targetとして登録（SLMモックfeatureを必須化）
- [x] 無効な辞書fixtureを実辞書に置換、thread戻り値のSend/Syncを修正
- [x] 日本語形態素/POS/byte・codepoint span、OOV、本番runtimeの無効GGUF拒否を検証
- [x] 開発PCのinit.defaultBranchに依存して失敗していたmergeテストを修正
- [x] CI設定で本番build・実NLP・モック試験を区別。Windows/Linux、実test target指定のTSan経路を設定

### 実行した検証

Windows上で`verify.ps1 -Offline`成功。本番cargo check、実資産target 3件、モック構成のunit 68件・workflow 3件・NLP 2件・並行性2件・doctest 1件が成功。期待テスト名の登録確認も成功。

online初回セットアップ、offline再セットアップ、PowerShell構文解析、隔離fixtureでarchive欠落・hash不一致時に停止し辞書を配置しないことを確認。

### 残作業と限界

P0はprogressを維持する。CI/TSanは設定変更のみでremote実行未確認。実GGUFによる推論精度・速度・tokenizer互換性は未検証。cfg(test)の暗黙モック置換の撤去、fake注入、tokenizerの危険なfallback除去、real-model fixtureは後続作業。MSVCリンク時の既存LIBCMT競合警告とsince_time dead-code警告は残る。