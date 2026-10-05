# Source-bound confidence review（2026-10-05）

`source_adapters::ConfidenceReviewRule`を追加。engine固有の閾値をcoreへ埋めず、呼出側が宣言した尺度・粒度・対象domainに一致するconfidenceのみ比較する。モデル・辞書・外部通信は不要で、OCR/ASR共通のRust APIとしてadapter moduleに置く。

```rust,ignore
let assessment = rule.assess(text, source, Some(domain), &evidence)?;
// assessment.review_requested は確認要求。認識誤り確定ではない。
// 各 observations[i].review_requested は Some(true)/Some(false)/None。
```

ruleには版付きid、recognizer（engine/model/version/decoder）、source profile全体（転記規約を含む）、domain、粒度、集約法、thresholdを指定する。identityやtargetが不明なruleは拒否。thresholdはRawScoreで、valueが境界、それ以外が適用対象の意味・方向・範囲・校正ID・target。意味/方向Unknownは拒否。raw値は書き換えず、数値の正規化や他の尺度への変換はしない。model/version/decoderを提供できない認識器は、この厳格なruleの対象外である。

入力evidenceの既存検証を先に実行する。不正score/spanはError。正常に構造検証できても、認識器・profile・domainが一致しない場合は全観測を適用不能にする。各観測はobserved、非空span、同じ粒度・集約法、同じscore descriptorのときだけ評価する。range未提供はruleも未提供の場合に限り一致する。

HigherIsBetterならvalue < threshold、LowerIsBetterならvalue > thresholdでreview要求。等値は閾値非該当。複数観測のいずれかが要求すれば全体review_requested=true。確率を加算せず、相関を独立扱いせず、依存IDを含むConfidenceObservationをそのまま返す。

- `Some(true)`: 明示境界を越えた確認要求。
- `Some(false)`: 境界を越えていないだけ。正しさ・低リスクの証明ではない。
- `None`: 欠測・尺度不一致等による適用不能。reasonを保持する。

空confidence集合・全体binding不一致にはunavailable_reasonを返す。falseという全体集約だけで受理せず、各観測の適用状態を参照する。本APIはRecognitionReportを変更せず、既存primary/candidate/sparse reviewの取消も行わない。risk確率・low_riskは生成しない。

2026-10-05の初期実装はsource evidence解釈の部品まで。その後facadeでのengine接続と融合レポートを追加した（次節）。代表データによる閾値採用は未実装。テストの0.5は人工契約専用で、OCR/ASRの推奨閾値ではない。高confidenceによる異常取消は今後も行わない。

提供PP-OCRv6 15件のconfidenceは方向・集約・targetが未定義で、本ruleの適用条件を満たさない。版・意味を推測で補完しない。新規データ収集やASR実行は行わず、必要なrecognizer定義・評価データはユーザーに別途依頼する。

認識core/schemaは変更なし。新Rust APIのみ追加し、既存adapter・旧reportはそのまま使用可能。small/core/fullの統計・POS観測にも変更なし。source契約テストはモデル/辞書なしでOCR/ASR、両方向、境界値、raw保持、欠測、版/粒度/集約/尺度/domain不一致、不正設定/入力拒否を検証する。
## Engine接続・統合レポート（2026-10-06）

```rust,ignore
let combined = rule.evaluate(&engine, input)?;
// 既に評価済みなら再解析せずに統合できる:
let combined = rule.combine(base_report)?;
combined.validate()?;
let json = serde_json::to_string(&combined)?;
```

`SourceReviewReport`は別namespace `kzn.recognition.source_review.v1`。`base`に無変更のRecognitionReport、`rule`に明示設定のsnapshot、`confidence`にraw保持のassessmentを格納する。`decision`が統合後の判断、`policy_id=kzn.recognition.source_review_or.v1`。base.decisionは従来のcore判断なので、統合判断が必要な呼出側は最上位decisionを参照する。

- baseがReview、またはconfidenceが確認要求ならReview（OR）。一次・候補・統計のどのreviewも取り消さない。
- 両方ともreview要求なしならUndetermined。高confidenceを正常・低リスクへ変換しない。
- source evidence未提供はconfidence=None、source_confidence_unavailableを理由に残す。部分欠測やbinding不一致も同理由と各観測の詳細reasonを保持する。reviewと適用不能の理由は共存できる。
- 推定済riskやLowRiskのbaseはこの未校正policyの範囲外として拒否する。確率融合はしない。

`evaluate`はruleを検証してからengineを1回呼び、`combine`は再解析しない。modelを追加せず、engine側の実行設定や元metricsは保持する。base.metricsの時間はcore処理の計測値であり、adapter統合時間を含む全体性能値とは扱わない。

JSONにはrule/assessmentの全フィールドを保存。unknown field拒否、`validate`はbase検証後に保存ルールからassessment・OR判断・理由を再計算して照合する。設定や原本の真正性・閾値の有効性を証明する検証ではない。出力改ざんを構造的に拒否するが、運用時のasset信頼管理は別責務。

旧core API/schemaは変更しない。新しいwrapperを既存recognition_qualityの観察JSONとして直接渡すことはできない。同CLIの既存品質結果はbase/core判断の結果である。source統合の品質評価runner・実際のsource定義に基づくrule採用は今後の作業。

人工契約でOCR/ASRの追加review、high-confidence+候補差のreview保持、source/domain欠落時の保留・理由、JSON往復/改ざん、設定不正を解析前に拒否、解析1回を確認。今回の追加で既存OCR 1kに閾値を適用したとは主張しない。