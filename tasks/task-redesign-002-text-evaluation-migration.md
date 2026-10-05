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

- [x] P0: 検証基盤復旧（2026-10-04完了）
- [x] P1: 評価契約・責務分離（2026-10-04完了）
- [x] P2: 一次検出MVP（2026-10-04完了）
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
## P0 継続作業（2026-10-04）

- [x] cfg(test)/mock_inferenceによる本番runtimeの置換を撤去。InferenceBackend traitでfakeを明示注入
- [x] 通常コンストラクタはfeatureに関係なく本番モデルをロード
- [x] 完全tokenizer JSONを渡すnew_with_tokenizer APIを追加。全語彙ID・embedding行数・EOS・architectureを検証
- [x] tokenizer欠落時のPAD/UNKフォールバック、GGUF token配列のJSON誤解釈を除去
- [x] 合成量子化GGUFで実CPU forward、繰り返しKV初期化、EOS停止、context超過拒否を確認
- [x] モック5msのlatency試験を削除し、最終decodeとcontext予算を修正
- [x] ローカル学習済みモデル向け参照token ID・生成・再現性の検証runnerとPowerShell入口を追加

学習済みGGUF資産の選定・参照ID照合とremote CI/TSan実行は引き続き必要。合成GGUFの成功を言語品質検証へ読み替えず、P0はprogressを維持する。
継続作業の検証: verify.ps1 -Offline成功。default unit 71件、実辞書/本番拒否3件、feature構成80件（unit71・workflow3・NLP3・並行性2・doctest1）成功。inference_smoke exampleのcargo checkも成功。

### CI・性能成果物の復旧

Draft PR: [#3](https://github.com/siska-tech/KazeNhanh/pull/3)。16fbaa2のLinux QA・TSanはremoteで成功。

CriterionがCRITERION_OUTPUTを読むconfigを追加。soakのrootをリポジトリ直下へ訂正し、stdout/stderrログと測定JSONを分けて保存。失敗を伝播し、終了時に環境変数と作業ディレクトリを復元する。CIは成果物欠落をエラーにする。

bench targetのcargo check、soakの0分起動、隔離fixtureで子プロセス失敗の伝播をローカル検証。性能値はモックworkflowでありSLM性能を示さない。

## P0完了（2026-10-04）

以下が現時点の結果。上記の「残作業」「remote未確認」は作業途中の記録。

- [x] 学習済みSmolLM2-135M-Instruct Q4_K_M（105,454,432 bytes）と公式tokenizer/config/model cardを固定revision・SHA256で取得
- [x] setup-model.ps1の初回取得、オフライン再利用、欠落・hash不一致拒否を確認
- [x] Python公式実装（transformers 4.46.3/tokenizers 0.20.3）で独立生成した日本語・混在文・絵文字/全角/結合文字・special tokenの4参照をRust tokenizers 0.19.1と照合
- [x] 公式chat template適用後（旧facadeと同じtrimを適用）のprompt IDも全4件一致。参照再生成のSHA256一致、誤ったtext/prompt IDの拒否も確認
- [x] Windows CPUの本番Candleで同一engineへ異なる4promptを入力し、全8推論の非空出力・各prompt再実行の一致を確認
- [x] 独立した実モデルCIジョブとtrained-model-smokeログ成果物を追加
- [x] 既存Windows/Linux QA、TSan、Criterion/Soakは[1e7589f CI](https://github.com/siska-tech/KazeNhanh/actions/runs/37177846413)で全成功。soakの5反復ログ・20測定JSONを取得確認済み

Windows検証: Core i7-1360P、Rust 1.99.0、debug build。8生成は26,794〜39,503ms（短い英語prompt、各最大128生成token）。debug smokeの時間であり、製品latency SLOや日本語品質の保証ではない。英語学習モデルはP0互換性fixtureとして使用し、日本語judge選定・判定品質・p95/RSS・常駐SLM soakは計画どおりP3/P4で扱う。

固定資産・参照生成・実行手順: [推論エンジン](../docs/inference_engine.md)、[参照fixture](../tests/fixtures/smollm2/README.md)。P0完了後の次工程はP1（評価契約・責務分離）。移行全体はprogressを維持し、P1〜P5は未完了。

## P1着手（2026-10-04）

評価契約とbackend traitをcoreへ新設し、Sudachi backend・legacy応用層・公開facadeをCargo workspaceで分離する。0.2開発版の変更として明記し、旧APIはlegacy feature/crateへ移行する。P1契約では検出未実装をundeterminedとして返し、P2の正常判定に読み替えない。

### P1契約の先行実装と分離の承認待ち

- [x] kaze_nhanh_coreを独立crateとして追加し、TextInput/Report/Error/config/byte span/三軸score/MorphAnalyzer/PrimaryDetectorを実装
- [x] 0.1の既存API/default featureを維持し、新評価APIだけを追加で再export
- [x] 原文/annotations保持、UTF-8境界、未評価/null、参照文脈不足、必須軸、schema/coverage、batchエラー分離のcore契約8件を検証
- [x] coreのbackend依存禁止とモデル/辞書なしの公開API起動を検証する入口をverify.ps1へ追加
- [x] REQ/API/ARCの002文書を追加
- [ ] Sudachi owned backend、legacy移設、workspace/feature matrix、0.2のAPI切替

自動承認レビューは「src全体の移設、Cargo workspace・依存関係・公開APIの広範な変更は、P1継続指示だけではこの具体的な非段階的構成変更への明示承認が不足」として、構成変更とビルド検証を拒否。既存構成を復元し、影響を限定した新規core/API追加の代替は承認されてbuild・契約試験が成功。全体分離・0.2 API変更の承認をユーザーへ確認中。P1は未完了。

P1先行実装の最終ローカル検証: verify.ps1 -Offline成功。core契約8件、公開API起動1件、default unit71件、実辞書3件、feature構成81件。coreのCargo依存はserde/serde_json/thiserrorのみ。既存0.1構成の回帰も成功。

### P1構成変更の承認（2026-10-04）

ユーザーから「進めてください」と明示承認を受領。core・Sudachi・legacyのcrate分離、0.2への切替、旧APIのlegacy feature化を再開。以下の結果で以前の承認待ち記録を更新する。

## P1完了（2026-10-04）

ユーザーがcrate分離・0.2切替・旧API feature化を明示承認。承認待ちは解消済み。

- [x] core/Sudachi/legacyをCargo workspaceへ分離し、root facadeを0.2開発版に変更
- [x] defaultは新Sudachiだけ、no-default-featuresはcoreのみ。通常依存のGit/Markdown/Candle/生成tokenizer混入をverify.ps1で拒否
- [x] owned辞書bytes/ローカルpath、Mode A/B/C、辞書/settings SHA256、全形態素のbackend非依存adapterを実装
- [x] 旧APIはlegacy featureでrootへ再export、またはlegacy 0.1 crateへ直接依存。旧errorは新EvaluationErrorから分離
- [x] モデルなしで起動するevaluate exampleを実行し、undetermined/null/SLM calls=0を確認
- [x] verify.ps1 -Offline成功。core8件、最小公開API1件、新Sudachi5件、legacy unit71件、旧NLP3件、workflow3件、並行性2件、doctest1件（全workspace計94件）
- [x] no-default+legacyのexample、mock_inference benchのcargo check成功
- [x] REQ/API/ARC-002・README・開発手順・旧推論手順・[0.2移行ガイド](../docs/migration-0.2.md)を同期

P2の標準検出器はまだ設定していない。形態素解析の成功を正常判定にせず、全文未評価を保持する。SLM judge/選択routingはP3で別backendとして実装し、legacy生成を評価経路へ接続しない。移行全体のstatusはprogress、end_dateは未設定のまま。

## P2着手（2026-10-04）

説明可能な文字・形式・反復ルール、全形態素features、用途別profile、JSON runnerと小規模baselineを実装する。OOV単独では異常化せず、意味整合性を未評価として保持。SLM呼出と訂正文生成は0。

## P2一次検出MVP（2026-10-04）

- [x] profile: required/Unicode scalar文字数/ASCII数字形式/禁止control/反復閾値/許容語、実行時snapshotと版管理
- [x] 一次rule: 文字化け候補、括弧不整合、日本語文字/全形態素の反復。助詞/助動詞を除外しない
- [x] OOV/正規化差/助詞/助動詞等を構造化featuresへ追加。OOV単独・累積costで異常化しない
- [x] 明示入力制約をinvalid、候補をsuspicious、必要な意味/参照を未評価/保留へ。二次disabled・SLM呼出0、訂正文フィールドなし
- [x] 根拠出力は256件＋省略集計に制限し、全入力検査と遅い位置の制約違反の根拠を維持
- [x] 4用途のJSON CLI/runner、34件の手作業fixture。全34期待一致、正常19件の誤警報0、意味保留4件、SLM呼出0
- [x] 初回baselineで見つかった許容語の形態素分割後の再警告を修正
- [x] Report schema v2とprofile_config/metrics.morphologyを文書化。Windows/Linux baseline成果物をCIへ追加

fixture SHA256: 673e88df0ce8284b9b8cab9bb221a618d950631fc63d89302084b1b6483112cd。小規模人工fixtureであり実運用品質のprecision/recall・校正済確率・SLMの品質を保証しない。次はP3（選択的SLM）。threshold/scoreと用途別品質受入はP4で実データ評価する。[一次検出仕様](../docs/primary-detection.md)。

ローカル最終検証: verify.ps1 -Offline成功（workspace全103件: core16・最小公開API1・新Sudachi6・legacy71・旧NLP3・workflow3・並行性2・doctest1）。default/minimal/legacy依存境界とcargo fmtを確認。最終baselineも34/34一致。remote CIはpush後に確認する。

## P3着手（2026-10-04）

backend非依存のSecondaryJudge/Factory、遅延load・有界worker、共有呼出予算・deadline、厳格出力検証と保留を実装する。実モデルの日本語judge・tokenizer bundle・CPU SLO検証は別の受入条件で、fake試験を実モデル品質の証拠にしない。

### P3制御契約の実装結果

- [x] SecondaryJudge/Factory、Arc共有worker、候補のみ遅延load、load失敗cache
- [x] worker累積呼出/byte/token予算、1worker＋有界queue、queue/loadを含むdeadline
- [x] reference不足は推論0、不正出力・故障・panic・timeout・queue飽和は保留
- [x] 期限切れ待機requestをcancel、実行開始の有無でslm_callsを記録。一次根拠を保持
- [x] 厳格JSON/軸/score/span/出力上限検証。暫定model scoreに校正済確率を付けない
- [x] schema v3、実Sudachi＋明示fakeのASR助動詞span接続、core secondary試験のTSan登録
- [x] verify.ps1 -Offline成功。workspace113件（core25・最小API1・新Sudachi7・legacy71・旧NLP3・workflow3・並行性2・doctest1）。最終core25件とbaseline34/34も成功、default CLIのSLM呼出0
- [ ] 日本語実モデルbackend・固定bundle/token ID照合・context/KV/協調deadlineの実CPU試験
- [ ] OCR/ASR品質・CPU latency/RSS/呼出率・事前SLO受入

ユーザー指定の優先用途はOCR/ASRの不自然さ検出。P3はprogressを維持し、実モデル試験をfakeの成功で代替しない。[制御契約と残作業](../docs/secondary-judging.md)。非協調backendの実行中処理をtimeoutで強制停止する保証はない。最新CIはpush後に確認する。
## P3実モデルadapter着手（2026-10-04）

OCR/ASR自然さ専用Qwen CPU adapterを独立optional crateへ追加。公式固定GGUF/tokenizer/config/licenseをhash検証。paired-label single-forward scoreは未校正、意味軸非対応。実CPU smoke・独立token ID・context/KV/timeout境界を検証する。P3全体はprogress。

### P3実験用adapterの検証記録

- [x] 独立optional Qwen CPU crate/qwen feature。default/minimal/coreへモデル依存を混入させない
- [x] 公式Qwen2.5-0.5B-Instruct Q4_K_M（491,400,032 bytes）・tokenizer/config/template/LICENSEをrevision/hash固定。online取得、offline再利用、破損cache拒否
- [x] Python公式chat templateの独立4参照をRustの全raw/prompt token IDと照合
- [x] 実CPU releaseで正常呼出0、異常候補完了、異なるprompt後のscore再実行一致、context超過保留
- [x] verify.ps1 -Offline成功。workspace116件（以前の113＋Qwen単体3）、qwen-only依存境界も検証
- [x] 専用セットアップ/verifyスクリプトと独立CPU CI、JSON成果物を追加

品質結果: 異常3例のnaturalnessは反復0.99986/文字化け0.82134/括弧欠落0.99728で、いずれも暫定0.5以上。この候補のOCR/ASR判定品質は未受入。一次warningは消さず、scoreを確率・採用済品質と称さない。14〜17秒/候補はローカルrelease smokeの参考値でSLOではない。P3全体はprogress。意味adapter・実OCR/ASR品質・CPU SLOを残す。[実験用adapter](../docs/qwen-judge.md)。最終版CPU/CI結果はIssue/PRにも記録する。
## 提供OCRデータの投入（2026-10-04）

PP-OCRv6 mediumの実認識5件を原文/confidence付きで追加。単一画像のdocument IDと未確認転記を別保存。cascadeと明示的なoffline全件比較でgate通過側も測る。confidenceを校正済評価scoreに変換しない。[サンプル仕様](../docs/ocr-samples-user-001.md)。

ユーザーが画像転記5件を確認。OCR不一致3・一致2。実投入ではcascade全5 acceptable/二次呼出0、不一致3件のgate見逃し3/3。offline Qwen比較も不一致3件すべて0.999以上で検出できず品質未受入。原文・source annotation/confidenceの保持をrunner内で検証。ライブラリrule/thresholdはこの画像へ後付け適合しない。

2026-10-04: 追加OCR 2画像/10件を原文・confidence・document ID付きで保存（計3画像/15件）。「体系キープ」「10kgやせる」の転記はユーザー確認済み、他8件は画像転記未確認。今回の転記差5件（確認済み1件）すべてが一次gateを通過。runner既定はモデル不要、SLM呼出/forward=0、Qwen比較は明示opt-inへ変更。P3品質未受入を維持。[追加データと観察](../docs/ocr-samples-user-002-003.md)。

検証: 3データセットの既定runner実行、原文/confidence/document ID保持、SLM呼出/forward=0、featureなしQwen比較の拒否、qwen付きexample compile、workspace tests・cargo fmt・git diff --checkが成功。

## recognition riskへの目的変更（2026-10-04）

ユーザー指定により、OCR/ASRの機械認識結果の異常・不確実性・誤認識riskを主対象へ変更。[再設計案002](../docs/KZN-REDESIGN-PLAN-002.md)を今後の計画として優先する。naturalnessは補助指標、Qwen自然さjudgeは実験比較用。既存P0〜P2実績とP3制御基盤は保持し、意味adapter/Qwen自然さ品質の受入を認識riskの必須経路から外す。

- [x] 現実装と15件のOCR開発観察を再監査し、残す基盤・変更する契約・低証拠時の保留・評価計画を文書化
- [x] README、ROADMAP、現仕様/移行手順に新方針と未実装の区別を反映
- [x] R0: RecognitionInput/Report、独立risk/adequacy/decision、証拠不足時のundetermined
- [x] R1: 型付きconfidence/candidates、OCR/ASR adapter/profile、欠測とalignment
- [ ] R2: lexical/string統計を加えた軽量baseline、独立実OCR/ASR評価集合
- [ ] R3: 選択LM surprisal/判別器、必要に応じclean/corrupted fine-tuning比較
- [ ] R4: source/domain別校正、低リスク受理品質・CPU SLO、移行リリース

今回の変更は設計文書のみ。Rust API・rule・score・モデル・15件の実測結果は変更していない。移行全体のstatusはprogress、end_dateは未設定。文書内リンク・差分形式を確認し、コードを変更していないため既存runtimeテストの再実行は省略する。

## R0契約・保留の実装（2026-10-04）

- [x] 別schema kzn.recognition.v1、新RecognitionEngine/Input/ReportとOCR/ASR source、必須document/segment ID
- [x] risk target/校正/転記policyとnull、evidence family/state、coverage/adequacy/decisionを独立
- [x] 無警告はundetermined、異常/制約はreview。低リスクpolicy未採用。naturalness未評価、モデル未接続
- [x] 原文/raw annotationsを保持、高confidenceを合格へ変換せず、入力制約を認識誤り確定と区別
- [x] モデル不要のSudachi facade / recognize CLI / 最小featureの公開API試験
- [x] coreの偽造拒否/原文/span/batch契約、実Sudachiの提供OCR15件とASR正常/反復、既存APIの回帰
- [x] verify.ps1 -Offline、依存境界、CLI実行、差分形式を確認

実測: 新R0経路では提供OCR15件が全てundetermined / risk=null / SLM呼出0。検出品質改善の主張ではない。R1のtyped confidence/N-best/profile、R2の語彙/文字列統計、R3/R4の推定/校正は未実装。[契約と利用](../docs/recognition-api.md)。移行全体はprogress、end_dateは未設定。

## R1 source evidenceの実装（2026-10-04）

- [x] confidenceの元f64値・意味・方向・range・粒度・集約法・calibration/target・欠測理由・依存ID
- [x] N-best rank/raw score成分・打切り/unknown・候補固有座標、bounded Unicode-scalar alignment
- [x] core共通型とfacadeのOCR bbox / ASR時刻・粒度adapter、source profile/必須signal/転記policy
- [x] 新report kzn.recognition.v2とtyped evidence整合検証。旧v1・不正source/span/summary/false completenessを拒否
- [x] 提供OCR15件のconfidence保持runner、参照/期待label非投入、候補/bboxを捏造しない
- [x] minimalでのsource契約9件とverify.ps1 -Offline成功。core/default/minimal/optional依存境界・全workspace回帰、formatと文書リンクを確認

提供OCR15件は全てundetermined / risk=null / SLM呼出0。confidenceはsegment粒度の未校正engine_score、方向/集約法/targetは不明として保持。実ASRデータは未取得、合成fixtureは契約試験のみ。risk推定・校正・モデル選定・検出品質受入はR2以降。[実装と互換性](../docs/recognition-source-evidence.md)。移行全体はprogress。

## R2統計evidence着手（2026-10-04）

- [x] 原文文字bigram・全形態素dictionary_form語unigram/bigram、未観測率・byte span・欠測分母
- [x] domain/辞書/settings/backend/mode照合、版付き疎頻度asset・出所/license/corpus hash、入力/entry/event上限
- [x] 一次と統計で1回の解析を共有。頻度/OOVからrisk/low_risk/reviewを自動生成しない
- [x] モデル不要clean corpus builder、SHA256照合runner、再現生成/不正asset/report/hash拒否
- [x] 人工8文の契約fixture、Unicode/bounds/改ざん/core契約6件、実Sudachiのhard clean・版不一致回帰
- [x] 提供OCR15件のローカル統計観察（学習への転記/label投入なし）: review=0、undetermined=15、low_risk=0、risk=null、SLM呼出0
- [ ] 文字種/POS等の追加特徴、異常rule・小さなfusionと比較baseline
- [ ] 独立した実OCR/ASR対、split/転記規約、検出/保留/低リスクの実数・品質比較

R2はprogress。人工clean assetは適用性能を保証しない。統計観察レポートのCIアップロード追加は自動承認レビューが拒否したため取り下げ、ユーザー由来の詳細はローカル保存にした。CIは統計生成とhash拒否を確認する。[統計契約](../docs/recognition-statistics.md)。移行全体のstatus/end_dateは未変更。
検証結果: verify.ps1 -Offline成功（core/default/minimal/qwen依存境界、core40件、source契約9件、実Sudachi9件、全workspace/legacy回帰）。統計6件の最終再実行も成功。R1 commit 2618e17の[remote CI](https://github.com/siska-tech/KazeNhanh/actions/runs/37204064451)は成功。R2のremote CIはpush後に確認する。

## R2辞書edition比較（2026-10-04）

ユーザー指定のsmall/core/fullを同版20250129で固定。archive/抽出dic hash、LEGAL保存、edition別配置・offline再利用、setup-dev -AllDictionariesを追加。builder/runnerに辞書path指定、辞書別統計asset ID、形態素snapshotとローカルmatrixを追加。人工hard-clean6例は学習8文と分離。提供OCR15件の形態素61/58/58、OOV3/3/3、全辞書undetermined15、判定変化0。R2品質受入は未完了、今後の実OCR/ASR・fusion/CPU比較にも3辞書を使う。[仕様](../docs/sudachi-dictionary-matrix.md)。詳細レポートはローカル保存。

検証: 3辞書matrixの最終実行、asset IDの3辞書分離、small資産/core入力の不適用、明示欠落path拒否、setupのarchive欠落/破損・抽出hash不一致拒否とLEGAL保存、全examples check、verify.ps1 -Offline、PowerShell構文・文書リンク・format/diffが成功。前回3b7bebbの[remote CI](https://github.com/siska-tech/KazeNhanh/actions/runs/37208894517)も成功。今回のremote CIはpush後に確認する。

## R2候補review baseline（2026-10-05）

- [x] opt-in raw候補不一致policy、候補rank・差分span・未知の正解性を示すfinding
- [x] 一次warningとのOR判断、欠測/同一候補は保留、risk=null・SLM呼出0、既定policy維持
- [x] 追加4契約試験: OCR/ASR、高confidence、挿入/削除/空境界、欠測/打切り、上限・改ざん拒否
- [x] 人工12例のoffline比較runner、3辞書全期待一致（review1→9、うち8件は候補差による追加）
- [ ] 独立実OCR/ASRの品質比較、転記規約・split、text/source/統合ablation・文字種/POS異常・学習fusion

表記ゆれ・句読点差もreviewになる比較baseline。候補の存在だけで正解を保証せず、提供OCR15件のgoldから候補を作らない。R2はprogress。[仕様と限界](../docs/recognition-candidate-review.md)。

検証結果: verify.ps1 -Offline成功。core40件、source契約13件（追加4）、実Sudachi9件、最小API・依存境界・全workspace/legacy回帰、3辞書のcandidate_baseline全12例、formatと文書リンク/diffを確認。前回0734f90のremote CIは成功。今回のCIはpush後に確認する。

## R2オフライン品質集計（2026-10-05）

- [x] モデル不要の事後評価器、report検証とID/原文/source/document一対一照合
- [x] 確認済み転記のみ採点、未確認別集計、版付き比較規約、入力SHA256記録
- [x] review/保留/low_riskと一致/不一致を交差集計し、分母0はnull
- [x] small/core/fullのmatrixへ接続、人工の集計・取り違え拒否テストをverifyへ追加
- [ ] 独立実OCR/ASR対・split・統合ablation・文字種/POS特徴・fusionと受入

[仕様](../docs/recognition-quality.md)。R2全体はprogress。評価器の追加であり、検出品質が改善したとは扱わない。

検証結果: verify.ps1 -Offline成功。追加4契約試験、small/core/full全matrix再実行、文書リンク・PowerShell構文・format/diff成功。3辞書とも確認済み7件（一致3/不一致4）、未確認8件、全15件保留。確認済み不一致review recall=0%、low-risk error rate=null。前回b988f1cのremote CIは成功。

## R2データ分割監査・公開サンプル取得（2026-10-05）

- [x] manifest/JSONL hash、一対一ID/source/document照合、版付き比較規約
- [x] document/origin/話者/session/非空転記hashのsplit跨ぎ拒否、未確認はdevelopmentのみ
- [x] 人工4件fixtureと4契約テスト、verifyへ監査CLI/テストを登録
- [x] GitHubの説明例1対・README/LICENSEを固定commit/hashで取得、再取得/Offline検証スクリプト
- [x] 初回取得、offline再利用、隔離人工fixtureの破損/欠落/path/source拒否
- [ ] 独立実OCR/ASRデータの採用、統合baseline・ablationと受入

[仕様](../docs/recognition-datasets.md)。公開説明例を実測やverifiedへ昇格せず、詳細原ファイルはtarget保存。取得時の自動承認レビューは当初利用上限で失敗したが、ユーザーの上限リセット後の指示で取得成功。移行statusはprogressを維持する。

検証結果: verify.ps1 -Offline成功、split監査4契約＋CLI、取得スクリプトの隔離offline契約、文書リンク/PowerShell構文/format/diff成功。前回75f129cのremote CIも成功。認識runtimeは変更せず、3辞書の品質値を新規実測として再計上しない。

## R2軽量統計review（2026-10-05）

- [x] 明示opt-inの統計三条件review、候補とのOR統合、欠測/不適用時保留
- [x] 全文scopeと件数/asset由来を保持、risk推定・高confidenceによる取消は行わない
- [x] 三条件/欠測/primary保持・JSON改ざん・候補＋高confidenceの追加3試験
- [x] 既存OCR15件を3辞書で比較するローカルrunner
- [ ] 代表clean corpus/追加特徴/source policy/学習fusionと独立品質受入

[仕様](../docs/recognition-sparse-review.md)。R2はprogress。追加サンプルはユーザーへ依頼し、ASR実行・外部収集を前提にしない。

検証結果: core43件（追加3）とverify.ps1 -Offline成功、正常型番の誤警報を明示する実Sudachi試験も成功。3辞書とも15件中review3/保留12（確認済み不一致のreview1/4、参照未確認review2）。正常型番ZX-900Bにもreviewが出るため実験用を維持し、既定採用/品質受入はしない。

## R2 POS bigram特徴（2026-10-05）

- [x] 隣接全POS vector統計・欠測・原文span・64 event上限
- [x] v2 asset明示生成、v1互換、全表entry上限とcanonical key/合計検証
- [x] 欠測橋渡し防止・学習POSなし・改ざん/上限/文書境界・解析1回の5契約テスト
- [x] builder --with-pos、辞書matrix -WithPos、v1/v2再現生成検証
- [ ] POSを用いた判断条件・文字種特徴・source policy/fusion・品質受入

[仕様](../docs/recognition-pos-statistics.md)。R2はprogress。追加サンプル取得やASR実行はしない。

検証結果: core48件（追加5）・v1/v2再現生成・3辞書matrix・最小API/source/実Sudachi等が成功。verify最終workspaceのWindows EXEロックは比較終了後の単独再試験で成功。POS未観測はsmall34/46、core/full33/43、欠測0。既定15件保留、risk=null/SLM0を維持。v1 assetのbyte/hash一致、format/リンク/PowerShell構文も確認。前回e859d49のCIは成功。

## R2文字種観測（2026-10-05）
- [x] 原文scalar固定分類・件数・遷移span・64件上限
- [x] 原文再計算検証・旧JSON欠落互換・OCR/ASR保留契約
- [ ] POS/文字種の判断条件・source policy/fusion・独立品質受入

[仕様](../docs/recognition-string-features.md)。statusはprogressを維持。追加サンプル取得・ASR実行なし。

検証: core52件（追加4件）成功。small/core/fullの15件すべてで文字種観測が一致し、既定review0/保留15・risk=null/SLM0を維持。前回dfb7db0のremote CI成功。

workspace全体（all-features/offline）の単体・統合・doc testsも全件成功。format/diff、文書リンク、PowerShell構文確認済み。

## R2 POS/文字種の条件比較（2026-10-05）
- [x] offline5条件・該当/非該当/適用不能・確認済み限定指標
- [x] 部分POS/旧資産/短文/旧report・AND欠測・改ざん拒否・元decision保持の追加4テスト
- [x] 既存15件をsmall/core/fullで比較。本番policy採用は見送り
- [ ] source policy/fusion、代表統計資産、独立品質受入

[結果](../docs/recognition-feature-ablation.md)。recognition_quality全8テストと3辞書runner成功。status=progress維持、追加データ収集・ASR実行なし。

全exampleのall-features/offline check、format/diff、文書リンク、PowerShell構文も成功。前回79892d0のremote CI成功。

## R2 source confidence契約（2026-10-05）
- [x] adapter明示rule・厳格binding・方向付き境界・raw保持
- [x] 欠測/不一致を適用不能とし、不正設定/入力はError
- [x] OCR/ASR/両方向/境界/欠測等の追加3契約テスト
- [ ] engine policy接続・融合出力契約・代表データでの閾値受入

[仕様](../docs/recognition-confidence-review.md)。source契約16テスト成功。推奨閾値なし、PP-OCR15件に未定義の意味を補完しない。status=progressを維持。

workspace全体all-features/offlineテスト、最小lib check、format/diff・文書リンク検証成功。前回3f2846eのremote CI成功。

## ユーザー提供OCR 1k登録（2026-10-06）
- [x] 5ファイルhash固定、既存split監査、行/manifest split照合
- [x] 全1,000件の推論allowlist/参照分離・原文/null保持・再現hash確認
- [ ] runnerのnull confidence対応、train限定統計asset、small/core/full development比較

[仕様](../docs/ocr-synth-1k.md)。status=progress。test条件検討・推論は未実施、再認識不要。

## OCR 1k development比較（2026-10-06）
- [x] null confidenceをMissing/Noneへ、0はObserved、定義をraw annotation保持
- [x] train500参照だけのPOS付きasset、small/core/full development100比較runner
- [x] 頻度比率JSON往復の再現テストとfloat_roundtrip修正、null契約CI登録
- [ ] source融合・代表データの受入条件、校正/test評価

[手順](../docs/ocr-synth-1k.md)。status=progress。calibration/testを推論へ渡さない。

比較結果: 3辞書とも既定保留100、sparse review24（不一致17/34、一致7/66）。POS ANDは拾える不一致も減少し、条件変更なし。6出力の全ID/原文/raw/null/SLM0/risk未算出を検証。core53件とrunner null契約成功。

workspace全体all-features/offlineテスト、format/diff、文書リンク、PowerShell構文も成功。前回f63ed9cのremote CI成功。

## Source review統合（2026-10-06）
- [x] rule evaluate/combine、core report保持、最終OR decision、欠測理由
- [x] 別JSON namespace、設定snapshot、assessment/判断再計算検証
- [x] OCR/ASR・高confidence取消防止・欠落・改ざん・解析1回の追加4テスト
- [ ] source定義の採用、統合品質runner、閾値と品質受入

[仕様](../docs/recognition-confidence-review.md)。source契約20テスト成功。status=progressを維持、既存datasetの新規推論・閾値調整なし。

最小lib check、workspace全体all-features/offlineテスト、format/diff・文書リンク検証成功。前回c32c90fのremote CI成功。

## Source統合品質集計（2026-10-06）
- [x] source envelope検証・最上位decision集計・base_summary併記
- [x] rule snapshot追跡/同ID異設定拒否・base特徴scope・旧形式互換
- [x] OCR/ASR、未確認、改ざん/重複/照合、旧dispatchの追加3テスト
- [ ] 実データsource定義に基づくrule採用・比較・品質受入

品質集計11テスト成功。status=progress、今回の変更でOCR 1kのconfidence品質を測定したとは扱わない。前回のIssue/PR同期は利用上限による自動承認レビュー失敗で未実行だったため、今回再開した。

全example all-features/offline check、保存済みdevelopment100件の旧CLI出力一致、format/diff・文書リンク成功。

## OCR source定義・development統合比較（2026-10-06）
- [x] engine commit/生成runner集約確認・runner hash固定・明示mapping
- [x] train観測scoreだけの下位10%境界とmethod/hash記録、train corpus再照合
- [x] small/core/fullでsource単独とsparse ORを比較、source/scale不一致拒否テスト
- [ ] 融合比較設計・独立品質受入・校正・CPU SLO

結果: confidence不一致9/34・一致0/66、ORは既存sparseの不一致17/34・一致7/66と同じ。追加検出0、条件変更なし。status=progress。

runner契約2件、全example all-features/offline check、3辞書比較、全600reportの原文/raw/欠測/review保持・SLM0/risk未算出照合、format/diff・PowerShell構文・文書リンク成功。

## 軽量学習fusion比較（2026-10-06）
- [x] train限定標準化・欠測indicator・固定logistic3特徴群・未校正margin
- [x] source/asset同一性とsplit/ID/原文重複検査、再現性等4テスト
- [x] 3辞書development比較。統合モデルは誤警報過多により未採用
- [ ] out-of-fold統計特徴・融合再比較・独立品質受入

[結果](../docs/recognition-fusion-baseline.md)。status=progress。confidence-only flag不一致26/34・一致1/66、統合flag不一致34/34・一致64/66。新たな閾値調整・calibration/test評価なし。

検証: fusion4テスト、全example all-features/offline check、3辞書runner、small再実行のbyte/hash一致、format/diff・PowerShell構文・文書リンク成功。前回c3cc23d CI成功。

## OOF統計特徴・融合再比較（2026-10-06）
- [x] document/origin/転記groupの5-fold、補集合corpus hash、各reportの資産binding
- [x] goldを含まない入力射影、精度を保つreport統合、追加3テスト
- [x] small/core/fullで同じ学習条件を再比較。統合flag不一致28/34・一致2/66
- [x] 外部生成コード更新と元データ不変を識別し、以前の監査済みsource定義を固定snapshotとして再利用
- [ ] 適用範囲/CPU費用・独立受入条件、校正とtest受入

[結果](../docs/recognition-fusion-baseline.md)。Status=progress。fusion全7テスト、全example all-features/offline check、3辞書matrix、small再実行hash一致、PowerShell構文とformat/diff成功。calibration/test推論・runtime変更なし。性能改善はdevelopment観察に限定する。
## 固定候補のscope/CPU監査（2026-10-06）

- [x] 候補/asset/report hash固定、再学習なしの予測再現と欠測/空入力別集計
- [x] release計測sidecarと3辞書×3回のCPU/ロード/メモリ観察、全report hash一致
- [x] 独立受入ゲートの対象・順序・未決要件を文書化
- [ ] 最終融合処理のCPU測定、運用上の数値要件、未評価区分のデータ充足、校正/test受入

[結果と範囲](../docs/recognition-fusion-scope-cpu.md)。Status=progress。fusion9テスト・runner2テスト・全example all-features/offline check成功。非空の不一致は20/26、空文字の8/8を別集計。runtime policy変更、再学習、calibration/test評価なし。