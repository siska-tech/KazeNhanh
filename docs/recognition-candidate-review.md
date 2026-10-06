# 候補不一致review baseline（R2先行実装）

2026-10-05。型付きN-bestを保持するR1契約に、明示的な運用判断を追加した。`RecognitionEngine::with_candidate_disagreement_review()`を呼ぶと、policy IDは`kzn.recognition.raw_candidate_review.v1`になる。既定は従来の保留policy。

```rust,ignore
let engine = japanese_recognition_engine(assets, RecognitionConfig::default())?
    .with_candidate_disagreement_review();
let report = engine.evaluate_recognition(input)?;
```

入力は既存のRecognitionInput.recognizer_evidence。adapterが検証したOCR/ASR共通の候補・alignmentを使う。OCR字形やASR音響scoreの解釈をcoreへ追加しない。confidence閾値・候補posterior・entropy・頻度閾値・モデルは使わない。

## 判断と根拠

一次warning/error、または提供候補のraw文字列不一致があればreview。後者は1-best以外の各候補についてcandidate_disagreement findingを最大15件返す（入力上限は16候補）。候補未提供・1件のみ・全て同一なら候補findingを生成せず、一次根拠がなければundeterminedを維持する。候補打切り、単一候補、高confidenceからlow_riskを作らない。

findingにはcandidate_rank、差分alignment block数、比較方式raw_text、score不使用、候補正解性unknownを記録する。spanは最初から最後の差分を囲む原文byte範囲で、間の同一文字を含むことがある。挿入だけなら空の境界spanを保持する。block数は入力alignmentの分割数で、最小編集距離や誤り件数ではない。訂正文・推奨候補・認識誤り確定labelは返さない。

例として自然な数値違いの候補があれば、どちらが原資料に一致するかを決めずreviewに送る。candidate_disagreementは既存Anomaly finding内の不確実性根拠として扱う。既存の入力制約findingと区別し、naturalnessは未評価、risk=null、assessment=insufficient_evidence、adequacy=limitedを維持する。一次と候補のreview理由も分ける。TextRules.finding_countには候補findingを加算しない。

**このpolicyは保守的な比較用baselineで、品質受入済みの既定policyではない。** 表記ゆれ・句読点・空白だけの差でもreviewになる。転記policyの正規化や候補選別は実装していない。通常N-bestには異なる候補が含まれるため、review率が高くなり得る。review数の増加をrecall改善と呼ばない。同一候補しか残らない認識誤りはこの方式では検出できない。

## 契約と検証

reportはkzn.recognition.v2を維持。既存の可変policy ID・finding code/evidenceを使用し、JSONフィールドやscoreの意味を変更しない。validateは既知policyの候補findingをtyped evidenceから再構築し、候補rank・span・件数・理由・severityの偽造、根拠の消去、保留への書換え、policy不一致、risk/low_riskの追加を拒否する。未知policyは品質検証済みと見なさない。

追加4契約試験はOCR/ASR、高confidenceと候補差、挿入/削除/空文字境界、候補欠測/同一/打切り、既存一次warning保持、上限とreport改ざんを検証。最小featureでも動作する。SLM呼出0、訂正生成なし。

[evaluation/candidate-review-contract.jsonl](../evaluation/candidate-review-contract.jsonl)は新規作成した12件の人工契約fixture（CC0-1.0）。数値・否定・同音候補・表記ゆれ・句読点等を含む。ユーザーOCRのgoldから候補を作っていない。baseline/candidate_reviewの期待値はrunnerの事後assertionだけに使い、推論入力へ渡さない。正解誤りlabelがないためprecision/recallは算出しない。

```powershell
cargo run --locked --offline --example candidate_baseline -- resources/sudachi/system.dic evaluation/candidate-review-contract.jsonl target/candidate-review-contract.json
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev/evaluate-dictionary-matrix.ps1 -Offline
```

small/core/full（20250129、Mode C）全てで12件の期待一致。既定policyはreview=1 / undetermined=11、候補review policyはreview=9 / undetermined=3。いずれもlow_risk=0、risk=null、SLM呼出0。8件の追加reviewには表記・句読点差も含む。verify.ps1はsmallの人工契約比較を実行し、matrixは3辞書を比較する。詳細はtarget配下に保存。

提供OCR15件にはN-bestがないため、このpolicyで検出成功したとは扱わない。R2の独立した実OCR/ASR集合、転記規約・split、text/source/統合ablation、文字種/POS異常・学習fusion、検出品質受入は引き続き必要。
