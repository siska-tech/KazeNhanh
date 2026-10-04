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