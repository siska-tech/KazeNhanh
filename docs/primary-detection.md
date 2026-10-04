# P2一次検出MVP

2026-10-04。`kzn.primary_rules.v1`はモデル不要の日本語screening。Sudachiの全形態素を入力順に使い、訂正文を生成しない。grammar/意味/事実性の完全判定器ではない。

## ルールと根拠

| code | 判定 | 対象 |
| --- | --- | --- |
| required_empty | invalid | profileで必須と宣言した空/空白入力 |
| max_chars_exceeded | invalid | profileのUnicode scalar文字数上限 |
| format_mismatch | invalid | 明示したASCII数字形式。外側の空白だけを無視 |
| forbidden_control | invalid | profileが禁止した制御文字。改行/CR/tabは許可 |
| replacement_character | suspicious | U+FFFD。欠落候補であり意図的使用もあり得る |
| unmatched_bracket | suspicious | ()、全角括弧、[]、{}、日本語引用括弧等の対応 |
| repeated_character | suspicious | 暫定6文字以上の同じ日本語文字。ASCII数字/英字/絵文字は対象外 |
| repeated_token | suspicious | 暫定3回以上の隣接する同一形態素。助詞・助動詞を除外しない |
| findings_truncated | info | 根拠256件＋省略集計に制限。全入力の検査と判定を継続 |

各issueは原文UTF-8 span、stage、構造化evidence、説明を返す。許容語は形態素surfaceと反復範囲の原文で照合し、分割の違いで許容指定を無効にしない。出力上限に達した後も、最初の確定的制約違反の根拠を残す。

OOV・正規化差・累積cost単独では警告/invalidにしない。metrics.morphologyにtoken/OOV/連続OOV/正規化差/助詞/助動詞の数を返す。costを確率やscoreへ変換しない。固有名詞・製品コード・短文・方言・fillerの正常例をfixtureへ含める。

## Scoreと保留

validityは宣言された制約への適合（0または1）、naturalnessは実装したwarning数に対する`max(0, 1 - 0.2 * warning_count)`。いずれもheuristicで、校正済確率ではない。naturalness=1は実装ルールが発火しなかったという意味で、文法的正しさの証明ではない。空入力やnaturalness無効profileはnot_applicable/null。

semantic_consistencyは評価しない。未評価はnullで、参照なしのreference scopeはinsufficient_context。defaultの必須軸はvalidity/naturalnessのみ。意味評価まで必要ならrequired_dimensionsへsemantic_consistencyを追加するかja.llm.v1を選ぶ。reference scopeを要求した場合は意味軸を必須へ含める。

確定的constraint違反ならinvalid、二次不要。warningならsuspicious、必要な軸/参照が未解決ならundetermined。二次候補はsecondary_needed=true/status=disabledとして残し、SLM呼出は0。正常化して推論率を下げない。acceptableもprofileと実装ルールの範囲に限定され、意味を合格扱いしない。

## ProfileとCLI

| ID | 初期方針 |
| --- | --- |
| ja.primary.v1 | 制約＋naturalness screening。意味は任意/未評価 |
| ja.ocr.v1 / ja.asr.v1 | 共通screeningの再利用。domain最適化済thresholdという意味ではない |
| ja.llm.v1 | reference scope・意味軸必須。P2では常に意味保留 |
| ja.form.v1 | required。naturalness無効、明示した入力制約を評価 |

DomainProfileを明示してrequired/文字数/数字形式/禁止control/反復閾値/許容語を変更できる。設定を変えたらprofile IDをversion管理する。未知ID・不正設定はfallbackせずError。実行時の全profile snapshotをprofile_config、rules/profile IDをprovenanceへ返す。source kindはmetadataであり、それだけでprofileを自動変更しない。

```powershell
cargo run --locked --example evaluate -- "今日は晴れですですです。" ja.asr.v1 asr
cargo run --locked --example evaluate -- "料金は100円です。" ja.llm.v1 llm "料金は200円です。"
cargo run --locked --example primary_baseline -- evaluation/primary-baseline.jsonl target/p2-baseline
```

baseline runnerは単一の共有Sudachi辞書で全ケースを実行し、reports.jsonl/summary.json、fixture SHA256、dict/settings hash、profile snapshot、caseごとの時間と集計を保存する。期待verdict/codeの不一致や意味保留の逸脱で非0終了する。CIはWindows/Linuxで実行しprimary-baseline-Windows/Linuxを保存する。

## Baselineと限界

34件の人工/手作業fixture（正常19、warning5、constraint5、意味保留4、未評価1）。全34期待一致、正常fixture誤警報0、意味保留4、SLM呼出0をWindowsローカルで確認した。fixture SHA256: `673e88df0ce8284b9b8cab9bb221a618d950631fc63d89302084b1b6483112cd`。

これは学習/校正/test分割した実運用の評価集合ではない。precision/recallや実データの誤警報率を保証しない。助詞欠落、流暢な意味矛盾、否定/数値の参照不整合等はこの少数ルールだけで確定しない。初期閾値とscore式は版管理する暫定値で、P3/P4で実データ・用途別品質とCPU SLOを評価する。

Windows Core i7-1360P / debugの参考測定: 辞書load約4.6秒、warm fixture p50約177µs/p95約536µs。小標本・短文・同一辞書の値であり、製品latency SLO/RSS検証ではない。

## Schemaと互換性

Report schemaを`kzn.evaluation.v2`へ更新し、profile_configとmetrics.morphology、一次判定に基づくroutingを明示する。v1 reportを黙ってv2へ解釈せず、必要なら呼出側で明示変換する。0.2は引き続き開発版。legacy 0.1の生成APIは変更しない。
