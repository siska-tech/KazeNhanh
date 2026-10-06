# R1: OCR/ASR source evidenceとadapter

2026-10-04。R1実装済み。R0の保留契約を維持し、recognizer由来の情報を型付きで受理する。risk estimator/校正・語彙統計・neural LMは未実装。

## 境界とAPI

coreは`RecognizerEvidence`、confidence/candidate/profile/anchorの共通型と検証、候補の文字列比較を持つ。公開facadeの`source_adapters` moduleがOCRのpixel bbox、ASRの秒単位時刻とsource別粒度を検証する。画像/音声認識runtimeは追加しない。facadeのJSON保持用serde_jsonはcoreでも利用する既存依存で、core/default/minimalへ推論依存を追加しない。

`RecognitionInput::new`の使用法は維持し、`input.recognizer_evidence = Some(&evidence)`で追加する。任意JSON annotationは引き続き原文spanとともに保持し、暗黙にconfidenceとして解釈しない。

```rust,ignore
use kaze_nhanh::*;
use kaze_nhanh::source_adapters::*;
let text = "認識されたテキスト";
let mut profile = RecognitionSourceProfile::new("my.ocr.v1", RecognitionSource::Ocr);
profile.required_signals = vec![RecognizerSignal::Confidence];
let evidence = adapt_ocr_evidence(text, OcrEvidencePayload {
    recognizer: RecognizerIdentity {
        engine: "my-ocr".into(), model: None, version: None, decoder: None,
    },
    profile,
    confidences: vec![ConfidenceObservation {
        id: "segment-confidence".into(), span: ByteSpan::whole(text),
        granularity: ConfidenceGranularity::Segment,
        status: RecognitionEvidenceStatus::Observed,
        score: Some(RawScore {
            value: 0.975, meaning: ConfidenceMeaning::EngineScore,
            direction: ConfidenceDirection::Unknown, range: None,
            calibration_id: None, target: None,
        }),
        aggregation: None, reason: None, dependencies: vec!["decoder".into()],
    }],
    candidates: CandidateEvidence::missing(), regions: vec![],
})?;
let mut input = RecognitionInput::new(text, RecognitionSource::Ocr, "document", "segment");
input.recognizer_evidence = Some(&evidence);
let report = engine.evaluate_recognition(input)?;
```

上のengineは[R0 API](recognition-api.md)のjapanese_recognition_engine等で組み立てる。source adapterはSudachiなしのminimal featureでも利用できる。

## Confidenceの意味と欠測

RawScoreはf64の元値、meaning（posterior/log_probability/logit/engine_score/unknown）、direction、任意range、calibration ID、推定対象targetを保持する。posteriorは0..1かつhigher_is_better、log_probabilityは0以下かつhigher_is_better。全ての元値/rangeは有限値を要求する。未知意味の値を1-confidenceや正解率へ変換しない。

calibration IDを持つsource scoreはposteriorと対象targetを宣言する。これは認識riskのcalibration IDとは別。token選択のposteriorと認識正解のposteriorを混同しない。校正artifactの品質や存在をこの構造検証が保証するわけではない。

ConfidenceObservationは原文span、字/token/語/行/発話/segment粒度、observed/missing/unsupported/invalid/failed/budget_skipped、集約法、欠測理由、依存IDを持つ。observedのみscoreを持ち、他stateはscore=nullと理由を要求する。値0と欠測を区別する。Characterは現契約ではUnicode scalar 1文字で、grapheme/UTF-16単位ではない。全行/発話のscoreを各文字に複製しない。

SourceProfileはsource・ID・任意転記規約とrequired_signalsを宣言する。これは品質受入済みthresholdのprofileではない。必須confidence/candidates/anchorsが欠けた場合はsummaryとreport理由へ残し、sufficient/low_riskを許さない。

## N-bestと位置対応

CandidateEvidenceはstate、候補列、truncated（未提供はnull）、origin、欠測理由を保持する。観測候補はrank 1から連続し、1-bestのraw textは入力と完全一致しなければならない。候補ごとのdecoder/acoustic等のscore component・意味・依存IDを保持する。score成分を合算・softmax正規化・entropy化しない。

adapterはUnicode scalar単位のLevenshtein alignmentを計算し、1-bestと各候補それぞれのUTF-8 byte座標を返す。正規化を行わない。挿入は1-best側の空span、削除は候補側の空span。全範囲を連続して対応させ、空文字同士は空alignment、同一文は全文のequal spanとする。最適解が複数なら対角→削除→挿入の順で選ぶ。この位置は文字列比較上の対応であり、音声/画像の真の対応や正解候補の証明ではない。

coreは候補差数・観測confidence数・unknown意味数・欠測・source anchor数をsummaryとして返す。候補が違うだけで誤認識を確定せず、現decisionは一次ruleのreviewまたはundeterminedを維持する。打切り候補内の分布を全探索空間のposteriorとは扱わない。

上限: 候補16件、候補本文合計65,536 bytes、非同一候補のDP matrixは262,144 cells/件・合計1,048,576 cells、confidence/anchor各1,024件、候補score成分16件。全候補をpreflightしてからDPを確保する。上限超過・不正値・rank/座標不整合はInvalidInputで、無言の切捨てや候補修正は行わない。

## Source固有の位置

OCR: zero-based pageとpixel単位 `[left, top, right, bottom]`。有限・非負・正の面積を要求する。ASR: callerのrecording/session原点に対する秒数。有限・非負・start<=end、非空本文は正の時間幅を要求する。source spanは原文byteで検証する。存在しないbbox/時刻・engine versionを推測しない。

coreはsource位置をopaqueなanchor dataとして保持する。直接RecognizerEvidenceを構築する呼出側は、adapterと同等のsource固有検証に責任を持つ。anchorがあっても原資料全体の完全性を評価したとは扱わない。現契約ではsource_completeness_assessed=trueを拒否する。

## Runner・互換性・検証

```powershell
cargo run --locked --offline --example recognition_samples -- evaluation/ppocrv6-medium-user-001.jsonl target/recognition-r1-001.json
cargo run --locked --offline --example recognition_samples -- evaluation/ppocrv6-medium-user-002.jsonl target/recognition-r1-002.json
cargo run --locked --offline --example recognition_samples -- evaluation/ppocrv6-medium-user-003.jsonl target/recognition-r1-003.json
```

runnerは認識原文・ID・engine・confidence・位置metadataのみを読み取る。参照転記・期待label・転記候補をruntime入力へ渡さない。recognizerのN-bestは提供されていないためmissing、bboxも未配置。confidenceはsegment粒度の未校正engine_scoreとして保持し、方向/range/集約法/targetを推測しない。

実投入15件は全てundetermined / risk=null / SLM呼出0。[観察JSON](../evaluation/recognition-r1-user-observations.json)。誤認識検出性能改善やASR品質の受入ではない。OCR/ASRのbbox・時刻・N-best・欠測fixtureは合成の契約試験で、実ASRデータを代替しない。

reportはtyped evidence追加に合わせ`kzn.recognition.v2`へ更新し、旧v1を同じschemaで解釈しない。RecognizerEvidenceのschemaは`kzn.recognizer.v1`。新v2 consumerでdeserialize後validateを行う。旧v1 JSONは拒否するため再評価が必要。RecognitionInputのstruct literalを使っていた場合はrecognizer_evidence=Noneを追加するかnewを使用する。旧kzn.evaluation.v3は変更しない。

検証はverify.ps1 -Offline（minimal/default/core/optional依存境界・全workspace回帰）とsource evidence契約試験、実Sudachiの15件runner、文書/形式検査で行う。次はR2のlexical/string統計とrisk baseline。

2026-10-05: with_candidate_disagreement_review()で、raw N-best差をreviewへ送るpolicyを明示選択できる。既定の判断とschema v2は維持。候補不一致を認識誤り確定・risk確率にしない。[候補review契約](recognition-candidate-review.md)。
