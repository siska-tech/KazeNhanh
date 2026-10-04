# Recognition API: R0契約と保留

> 2026-10-04: R1でtyped confidence/candidates/adapterを追加し、report schemaをkzn.recognition.v2へ更新。以下はR0の判断契約の説明。[R1と移行事項](recognition-source-evidence.md)。

2026-10-04。実装済みのR0。[再設計案002](KZN-REDESIGN-PLAN-002.md)の最初の段階。OCR/ASR用のモデル不要APIを追加し、無警告を低リスクへ変換する経路を持たせない。

## 利用

```powershell
cargo run --locked --offline --example recognize -- "節約する" ocr image-001 line-02
cargo run --locked --offline --example recognize -- "猫猫猫猫猫猫" asr session-001 utterance-01
```

前者はundetermined、後者はreview。どちらもrisk=null、evidence_adequacy=limited、SLM呼出0。モデル資産は不要。Sudachi辞書のセットアップは[開発手順](development-setup.md)を参照。

```rust,ignore
use kaze_nhanh::*;
let assets = SudachiConfig::from_paths("system.dic", "sudachi.json", SudachiMode::C)?;
let engine = japanese_recognition_engine(assets, RecognitionConfig::default())?;
let input = RecognitionInput::new("認識されたテキスト", RecognitionSource::Ocr, "image-001", "line-01");
let report = engine.evaluate_recognition(input)?;
report.validate()?;
```

`default-features=false`でもcoreのRecognitionEngineに任意MorphAnalyzerを注入できる。evaluate_recognition_batchは順序を保ち、入力ごとにResultを返す。document/segment IDは必須、sourceはOcr/Asrのみ。日本語、max_input_bytes、UTF-8 byte spanの制限とbackend contract検証を既存基盤から再利用する。

## 出力と判定

現JSONは`kzn.recognition.v2`（R0時点のv1から更新）。従来の`kzn.evaluation.v3` / evaluateとは別契約。

| フィールド | R0の意味 |
| --- | --- |
| recognition_risk | target=segment_contains_transcription_error。高いほど危険な確率用の型。R0はvalue/method/calibration_id=null、status=insufficient_evidence |
| transcription_policy_id | R0は未設定/null。将来の確率推定では転記規約のIDを必須とする |
| evidence_adequacy | limited。形態素/文字ルールの証拠はあるが、認識risk推定を完了できない |
| decision | 一次warning/errorがあればreview、他はundetermined。R0はlow_riskを出さない |
| evidence | morphology/text_rulesはobserved、recognizerはmissingまたはunsupported、lexical_statistics/language_model/risk_estimatorはunsupported |
| findings | anomalyとinput_constraintを区別。制御文字等のprofile違反も確認対象だが、誤認識確定としない |
| coverage | 入力文字列の処理範囲とrisk評価範囲を分離。全文処理済でもriskは全文未評価、原資料の完全性は未評価 |
| naturalness | 補助欄。R0は未評価/null。旧screeningのheuristic=1をコピーしない |
| decision_policy | kzn.recognition.abstain.v1。低リスク閾値と校正IDは未設定 |
| metrics/provenance | 形態素features・辞書/設定/rule/policy識別、SLM呼出0 |

typed source evidenceを渡した場合はrecognizer familyをobservedとして返す。詳細はR1文書を参照。SourceAnnotationを原文spanとともにそのまま保持する。raw JSON内のconfidence・候補・bbox等は解釈しない。annotationがあればrecognizer evidenceはunsupportedで理由を残す。何もなければmissing。高いraw confidenceを確率や低リスク根拠へ変換しない。

厳格JSON deserializeに加えて`RecognitionReport::validate()`が必要。未知schema/field、不正span、evidenceの重複/欠落・state/value不一致、未推定のrisk値、偽の評価coverage、根拠不足のlow_riskを拒否する。将来のlow_riskにはcalibrated method・校正ID・転記policy・risk estimatorの観測状態・十分な証拠・risk評価範囲・同じ校正IDのdecision policy・閾値内risk・未解決warning/errorなしを要求する。構造検証はモデル品質や校正の正しさを証明しない。

## 現実装の境界

既存のPrimaryRulesを証拠抽出に利用し、そのacceptableとnaturalness scoreは新判断へ転用しない。R0時点ではsource固有profile、型付きconfidence/N-bestは未実装だった。R1で追加済み。R2で語彙/文字n-gramの統計evidenceを先行追加した。[統計資産と適用条件](recognition-statistics.md)。統計による異常検出/fusion、risk estimator、LM scorer、校正は未実装。旧SecondaryWorker/Qwenを新engineへ接続するAPIは持たない。

旧evaluate APIはそのまま利用可能だが、OCR/ASRの新規利用はrecognition APIを推奨する。R0で保留できることと誤りを検出できることを区別する。R1でconfidence/candidatesの型、adapter/profile、欠測とalignmentを追加済み。次はR2の軽量統計baseline。

## 検証

- core契約試験: OCR/ASRの無警告、自然な誤認識候補、空文、原文/Unicode保持、raw高confidence、異常と制約の分離、JSON/schema/span、低リスク/確率/coverageの偽造拒否、batchの失敗分離。
- 最小facadeのモデル/辞書不要試験と、実Sudachiによる提供OCR15件の原文/confidence保持・全件保留、ASR正常/反復例を確認。
- verify.ps1 -Offlineが成功。default/minimal/core/qwen依存境界、現API・legacyの回帰を検証。最後に追加した低リスク閾値/校正一致の契約試験を含むcore34件も成功。
- CLIのOCR無警告とASR異常例を実行し、risk=null/SLM呼出0を確認。

OCR15件の全件保留は検出性能の改善を意味しない。実ASR品質は未評価。未校正model/risk score、低リスク受理率、CPU SLOの受入はR2以降に残る。

2026-10-05: with_candidate_disagreement_review()で、raw N-best差をreviewへ送るpolicyを明示選択できる。既定の判断とschema v2は維持。候補不一致を認識誤り確定・risk確率にしない。[候補review契約](recognition-candidate-review.md)。
