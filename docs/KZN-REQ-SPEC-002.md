# KZN-REQ-SPEC-002: ローカルテキスト評価基盤

2026-10-04。旧001要件はGit/要約応用層の履歴。新基盤の要件と段階はKZN-REDESIGN-PLAN-001、API契約はKZN-API-SPEC-002を優先する。

1. detection first。訂正文生成を評価APIの責務に含めない。
2. 原文・全形態素・byte spanを保持し、OCR/ASR/LLM/Formのテキストを共通APIで受理する。
3. Sudachiによる一次判定を標準とし、SLM資産がなくても起動可能にする。
4. validity/naturalness/semantic_consistencyを共通score形式で返し、未評価・文脈不足・失敗を正常へ変換しない。
5. CPU-first/local-first。資産を明示ロードし、実行時の自動download/cloud fallback/入力収集を行わない。
6. Git・Markdown・RAG・生成要約をcoreのCargo依存、公開error、起動条件から除く。
7. SLMはP3で不自然・曖昧な候補へ限定し、予算と保留を設ける。P1のSLM呼出数は0。
8. Fine-tuning/model adapter/calibrationは独立artifact/backendとして後から拡張する。学習処理はruntimeへ入れない。
9. calibration前のscoreを確率と称さず、三軸を無条件に平均しない。日本語の判定品質とCPU SLOはP2 baseline後に定める。

P1受入: backend非依存core、モデルなし起動、原文/span/未評価/schema契約試験、legacy境界、仕様の同期。ユーザーの承認後、core/Sudachi/legacyを分離して0.2へ切り替えた。
