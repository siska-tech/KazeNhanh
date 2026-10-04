# KazeNhanh 再設計監査・移行計画

- 文書ID: KZN-REDESIGN-PLAN-001
- 作成日: 2026-10-04 (Asia/Tokyo)
- 監査対象: commit `ddea985`、Rust crate `kaze_nhanh` 0.1.0
- 状態: 初回監査・設計提案。監査後、[Issue #2](https://github.com/siska-tech/KazeNhanh/issues/2)でP0に着手。現在の検証結果・残作業は[移行タスク](../tasks/task-redesign-002-text-evaluation-migration.md)を参照。以下の監査結果は変更前の記録。
- 対象: src、Cargo設定、tests、benches、CI、既存仕様・README・ロードマップ。

## 1. 結論と境界

新しいコア責務は「入力テキストの問題を検出し、根拠・評価範囲・不確実性を伴う評価を返す」こととする。既存の要約パイプラインを名前だけ変更する移行では成立しない。SudachiラッパーとCPU推論の一部を再利用し、評価契約、一次判定、routing、scoring、評価データを新設する。

MVPは日本語のテキスト評価に限定し、OCR/ASRエンジンそのもの、音声・画像の直接評価、修正文生成、Git履歴取得、要約、ネットワーク検索はコアに含めない。入力元は拡張可能とし、既存のGit/Markdown機能は別の応用層へ移す。CPU-firstであり、将来のバックエンド拡張まで型で禁じるCPU-onlyにはしない。

重要な制約: 形態素が自然でも、否定・数値・主体・時制の食い違いはあり得る。低OOV率だけで意味整合性を合格にしてはいけない。参照文脈と照合する必要があるのに一次判定で解決できなければ「意味判定が曖昧」としてSLM候補にする。全件の意味整合性保証と、全件SLM回避を同時に無条件で約束しない。

## 2. 現実装の監査

以下の行番号は監査時点のコード。P0は評価結果への信頼を損なうもの、P1は移行上の主要な障害、P2は分離先で扱うもの。

| 優先度 | 根拠 | 確認事項・影響 | 対応 |
| --- | --- | --- | --- |
| P0 | `Cargo.toml`、`tests/integration/api_workflows.rs`、`tests/concurrency/thread_safety.rs` | 結合・並行性テストはサブディレクトリ内にあるが、入口main.rs、上位mod宣言、明示的test targetがない。cargo metadataにもtest targetがない。CIのテスト名フィルタだけでは実行されない | 入口またはtest targetを登録し、期待テスト名と実行件数をCIで検査 |
| P0 | `src/inference/model.rs:1,376`、`.github/workflows/testing.yml:39` | cfg(test)またはmock_inferenceで本番runtimeを置換。all-featuresもモックを有効にする。本番推論のビルド・品質をこのテストでは確認できない | traitによるfake注入と本番バックエンドのビルド・実モデル試験を分離 |
| P0 | `src/inference/model.rs:225,280,307` | tokenizer復元不可時にPAD/UNKのみのWordLevelへ黙ってフォールバック。通常のGGUF tokensは文字列配列だが、当該コードはJSON文字列または整数バイト配列扱い。モデルとtoken IDの対応を保証できない | 正式tokenizer資産を明示ロードし、非対応形式・不一致はエラー。日本語の既知token ID列で照合 |
| P1 | `src/lib.rs:92,101,482`、`Cargo.toml` | facadeがNLP・SLM・要約・Git RAGを所有。起動時にSLMロード必須。依存のgit2、Markdown、Candleも必須。NlpService単体は呼べるが設定型はmodel_bytes込み | 評価エンジンと設定・エラーを分離。SLMは任意、遅延ロード。Git/要約は別crate |
| P1 | `src/pipeline/hybrid_summarizer.rs:79,116` | 抽出後の有効な要約実行は必ず生成推論。異常度・曖昧度によるgateがない | 評価専用の一次判定・routingを新設 |
| P1 | `src/pipeline/sentence.rs:107,152,218` | 短文除外、内容語中心、stopword除去、語頻度制限は要約用途。助詞・助動詞・語順・反復など評価に必要な情報を失う | 評価経路で流用せず、全文・全形態素・出現順を維持 |
| P1 | `src/foundation/nlp_service.rs:39,127,189` | POS、OOV、辞書形、正規化形、読み、原文byte/codepoint位置、累積costが得られる。ただしMode C固定で、評価器はない | ラッパーを再利用。Modeを設定化し、辞書・設定versionを記録。costを確率と扱わない |
| P1 | `src/lib.rs:168,174,244` | 公開出力は要約String、文リスト、形態素。妥当性・自然さ・意味整合性・issue・abstentionは未実装 | 評価結果schemaとevaluate APIを新設 |
| P1 | `src/inference/model.rs:14,79,125,150` | 生成用の固定長・温度・top-p。入力の先頭を切り捨て、token VecのdrainはあるがKV cacheを同時に切り詰める処理は見当たらない | 判定用出力・token予算を設計。長文は明示分割し、KV/位置管理は実モデル境界試験で確認。現時点では実行時不具合の再現未確認 |
| P1 | `tests/integration/api_workflows.rs:12`、`tests/concurrency/thread_safety.rs:15` | fixtureはb"dict"を使うが、NlpServiceは実辞書を読む。登録を直しても初期化成功は期待できない。並行性fixtureのBox<dyn Error>もthread戻り値のSend要件に抵触する見込み | fake NLPと実資産テストを分け、登録後に型検査と実行を復旧 |
| P1 | `benches/performance.rs`、`docs/inference_engine.md` | 性能測定はモック。Git benchは各反復でrepo作成も含む。実日本語辞書・SLMのCPU性能、検出精度、calibrationの裏付けはない | 段階別benchとラベル付き評価を新設 |
| P2 | `src/lib.rs:220` | extract_important_sentencesは先頭count文を返し、LexRankを使わない | legacy側で仕様名と実挙動を整合 |
| P2 | `src/foundation/git_service.rs:191,212`、`src/pipeline/git_native_rag.rs:122` | 履歴の追加行番号を現在のworktree本文に対応付ける。後続編集によって節の誤対応やファイル不在が起き得る | Git応用層でcommit/blob由来の本文と座標を揃える |
| P2 | `scripts/soak/run_soak.ps1:10` | projectRootがscriptsを指すためログはscripts/artifacts側。CIのartifacts/soak指定とずれる。繰り返し別プロセスのbenchは同一モデル常駐時のメモリsoakを代替しない | ログ配置と常駐モデルsoakを再設計 |

GitNativeRAGのfallbackメッセージ自体は要約用途の挙動として存在するが、評価APIの「問題なし」に流用しない。READMEのproduction品質・旧ROADMAPの100%は旧タスク記録であり、新基盤の完成度や今回の実行検証の証拠ではない。

### 再利用・移設・新設

| 部分 | 方針 |
| --- | --- |
| NlpService / TokenizedMorpheme | 再利用の中心。EngineConfig依存を外し、backend固有型を公開契約から隠す |
| split_sentences / saku | 原文spanを返す設計に改修。小数、URL、改行、引用、絵文字、短いフォーム値を再評価 |
| Candle CPUロード | 候補として残す。tokenizer互換性と実モデル試験を先に通し、採用可否を決める |
| Arcによる辞書共有 | 再利用。SLMのMutexは有界worker/queueと予算制御の背後に置く |
| Markdown / Git / 相関 / prompt_builder | legacy応用crateへ移設 |
| HybridSummarizer / LexRank | legacy要約へ移設。重要度scoreを妥当性scoreへ転用しない |
| 評価型 / rule engine / gate / calibrator / 評価データ | 新設 |

## 3. 目標アーキテクチャ

依存方向は応用層 → facade → core。backend実装はcoreのtraitを実装し、coreはSudachi/Candle/Gitを知らない。

```text
OCR / ASR / LLM / Form / Plain text / 任意のGit入力adapter
        ↓ TextInput + optional context + source metadata
原文を保持した文・領域分割（byte spanとsource mapping）
        ↓
Sudachi等 → FeatureExtractor → PrimaryDetector
        ↓ findings / raw evidence / uncertainty / coverage
RoutingPolicy ── 十分な根拠あり ───────────┐
        ├── 疑わしい・曖昧 → 有界SLM judge ─┤
        └── 無効化・上限・失敗 → 未評価/保留 ┤
                                          ↓
                           ScoreFusion → Calibrator → EvaluationReport
```

推奨する最終構成（最初から空crateを大量作成せず、P1で必要な境界だけ抽出）:

- `kaze_nhanh`: 公開facade。evaluate/evaluate_batch、設定、任意backendの組み立て。
- `kaze_nhanh_core`: input/report、span、features、rules、routing、scoring、traits。Git・生成ライブラリに依存しない。
- `kaze_nhanh_sudachi`: Sudachiと辞書資産。標準の日本語backend。
- `kaze_nhanh_slm`: 任意のローカルjudge。初期候補Candle、別runtimeへ交換可能。
- `kaze_nhanh_legacy`: Git/Markdown/RAG/要約APIと固有error。必要になれば後から細分化。
- `evaluation/` または開発専用crate: dataset runner、bench、calibration fitting。通常runtimeに学習依存を入れない。

標準配布はSudachi一次判定が動き、SLMはopt-in。no-default-featuresでcoreとfake backendを検証できるようにする。単なるmodule移動では依存重量は減らないため、Cargo依存・再export・error内のgit2/Candle型まで分離する。

## 4. 入出力契約

API名・型は提案。MVPで確定し、schema_versionを付ける。

```rust
pub fn evaluate(&self, input: TextInput<'_>) -> Result<EvaluationReport, EvaluationError>;
pub fn evaluate_batch(&self, inputs: &[TextInput<'_>]) -> Vec<Result<EvaluationReport, EvaluationError>>;
```

TextInputにはtext、language、source kind、domain/profile、任意のcontext/reference、source annotationsを持たせる。OCRの文字confidence/bbox、ASRの時刻/confidenceはadapter側で原文spanと対応付け、未提供値を推測で埋めない。input formのrequired/format等はprofileで明示する。LLM評価の参照文書も呼出元が渡す。

EvaluationReportは次を返す:

| フィールド | 契約 |
| --- | --- |
| verdict | acceptable / suspicious / invalid / undetermined。運用policyによる集約結果 |
| scores | validity / naturalness / semantic_consistencyの共通DimensionScore型 |
| DimensionScore | value: Option<f32>（0..1、高いほど良い）、status、method、scope、calibration_id、任意confidence。未評価はnullで0.5や1.0にしない |
| issues | code、severity、原文span、検出stage、構造化evidence、説明。訂正文は含めない |
| routing | SLM必要性、実行有無、理由、disabled/budget_exceeded/timeout/backend_error等 |
| coverage | 評価した軸・範囲、未評価span、文脈不足、長文分割による制限 |
| provenance | 辞書/設定/rules/model/tokenizer/prompt/adapter/calibratorの識別子とhash、schema/feature version |
| metrics | stage別時間、token数、SLM呼出数、必要ならqueue待機。原文ログは既定off |
```

- validityは宣言された入力制約とテキストとしての成立性、naturalnessは指定言語・文体での自然さ、semantic_consistencyは指定scopeでの矛盾/整合性とする。外部世界の事実性を一括で保証する名前にしない。
- semantic scopeはinternal（文内・文間）とreference（与えた文脈との整合）を区別する。参照なしではreferenceはnot_applicableまたはinsufficient_context。自然でも参照と矛盾する文を低risk扱いしない。
- 三軸は共通形式で返すが、違う意味の値を単純平均しない。総合scoreはMVPでは任意とし、導入時は重み・必須軸・欠測時の扱いをprofileでversion管理する。
- 初期のrule scoreはheuristicと明記し、確率と呼ばない。SLMの自己申告confidenceもそのまま確率にしない。calibration後だけ、対象集団とラベル定義に沿う推定値として扱う。
- API呼出不備・辞書初期化失敗はError。正常に受理した文章の問題はReport。任意SLMが使えない場合は一次結果を残してpartial/undeterminedとする。厳格profileでは必須軸未評価のacceptableを禁止する。
- spanは原文UTF-8 byteの半開区間を正とする。UI向けUTF-16、codepoint、bbox、時刻への変換はadapterで行う。全文正規化をするなら可逆な位置対応表を保持する。

## 5. 一次判定とrouting

一次判定は形態素解析に加え、軽量な文字・形式・文脈規則を使う。Sudachiのみで構文正誤や意味の正解を確定する設計にはしない。

初期特徴: OOV率と連続OOV、品詞列・活用・助詞/助動詞の並び、文字種の不自然な切替、文字/語の反復、括弧不整合、原文と正規化形の差、数値/単位/否定表現とreferenceの不一致候補。OOVは固有名詞・新語・製品番号でも増えるため単独でinvalidにしない。数値差も表記ゆれ・単位換算を考慮し、曖昧なら候補に留める。

Sudachi v0.6.9のtotal_costは経路先頭からの累積値であり、単語ごとの誤り確率ではない。まずcostを使わないbaselineを作り、採用時は文長・辞書・Mode・domainを揃えた特徴として検証する。Mode A/B/Cも実データで比較し、一律の最適値を仮定しない。[該当バージョンの実装](https://raw.githubusercontent.com/WorksApplications/sudachi.rs/v0.6.9/sudachi/src/analysis/morpheme.rs)

RoutingPolicyは「品質score」「判定の不確かさ」「評価coverage」「実行予算」を分離する:

1. profileの確定的制約違反は一次でinvalidにでき、SLMは不要。
2. 必要な軸・scopeについて一次評価に十分な根拠があるなら終了。これは全意味の正常保証ではない。
3. 不自然さ候補、一次信号の衝突、文脈整合性が未解決ならSLM候補。
4. 候補だけを優先度順に処理し、max_input_tokens/max_output_tokens/max_calls/deadline/queue上限に従う。
5. 候補が予算を超えたら残りは保留。SLM率を下げるために正常へ書き換えない。すべてが曖昧なbatchでは全件候補になり得るが、全件推論は強制しない。

thresholdは異常検出recallとfalse-positive、SLM呼出率の曲線から決める。任意の固定値を品質保証として出荷しない。形態素を通過する意味誤りを評価datasetに必ず入れ、一次で見逃した例も含む全件でend-to-end recallを算出する。

## 6. SLM・資産・CPU予算

SLMの責務は指定された文と必要文脈に対するjudge。長い説明や修正文を生成させず、有限labelまたは短い構造化結果と根拠spanを返す。生成JSON方式ならschema/範囲/spanを検証し、出力不正は未評価として処理。対象テキスト内の命令に従わないtemplateと耐性試験を用意する。自由文を数値として解釈しない。

既存のquantized_llama採用は暫定。GGUFという拡張子だけで全architectureに対応できるとは扱わない。model architecture、量子化、tokenizer、BOS/EOS、chat template、context上限をbundle manifestで検証する。GGUF仕様ではtokenizer.ggml.tokensはarray[string]であり、現行のバイト列復元とは一致しない。[GGUF仕様](https://github.com/ggml-org/ggml/blob/master/docs/gguf.md)

モデル選定は日本語判定精度と対象CPUで行う。まず互換性が確認できる小型量子化モデル候補を比較し、サイズだけで確定しない。語彙・template・license・配布条件を候補ごとに記録する。必要ならbackendを交換するが、runtimeを先に全面書換えしない。

- AssetSourceは静的bytesに加えローカルpath/owned bytesを検討。mmapはbackendの対応と実測で決定。自動downloadやcloud fallbackはしない。
- SLMは初回route時だけロードし、bounded workerで再利用。一次のみならモデル未配置でも動作し、モデルメモリを確保しない。
- tokenizer、prefill、decode、queue待ち、cold load、warm推論、RSSを分けて計測。CPU thread数を制限し、形態素workerとの過剰並列を防ぐ。
- 長文はspan付きchunkと前後文脈を用い、未評価範囲を記録。指示・参照の無言切捨ては禁止。chunkを跨ぐ整合性を評価できない場合はcoverageで表す。
- deadlineはtoken間で確認する協調中断が初期案。計算kernel中の厳密な停止保証は別問題なので、硬い期限が必要ならプロセス分離を検討する。
- cacheは後続最適化。使うならtextだけでなくcontext/profile/model/tokenizer/dictionary/calibrator versionをkeyに含め、容量を制限する。

## 7. Fine-tuning / adapter / calibrationの拡張点

初期trait境界はMorphAnalyzer、PrimaryDetector、RoutingPolicy、SecondaryJudge、ScoreCalibrator。アプリ用のSourceAdapterとモデル用のModelAdapterを区別する。

- DomainProfile: 辞書・許容語・rule・threshold・評価軸・文体・参照要件をまとめる。
- ModelBundle: base model、tokenizer、template、任意のadapter、互換versionとhash。
- ModelAdapter: backendが対応する場合のLoRA等。未対応backendなら学習後にmerge/exportした別bundleとして読み込める設計にする。Candle/GGUFで動的adapterが既に動くとは約束しない。
- Calibrator: raw features/SLM evidenceを共通scoreへ変換する独立artifact。小さい線形モデル等を候補にして、CPU負荷とcalibrationを検証する。
- 学習処理はオフライン開発toolへ分離。runtimeにoptimizerや学習データを同梱しない。利用者テキストを自動収集しない。
- rules、辞書、モデル、quantization、routingを変えると分布も変わるため再評価・必要に応じ再calibration。対象外domainには未校正表示または保留を返す。

## 8. 段階的移行と完了条件

工期は実資産の確保・対象CPU・必要精度が未確定なので固定せず、次の完了条件で管理する。最初の実装はP0とP1、最初の有用なリリースはP2まで。

| 段階 | 実施内容 / 主な変更先 | 依存 | 完了条件 |
| --- | --- | --- | --- |
| P0 検証基盤復旧 | Cargo test登録、fixture注入、real/mock分離、CI、bench分類、旧品質記述の訂正 | なし | 期待する結合・並行性テストの検出と実行。defaultの本番buildとmock testを別々に通す。実辞書・tokenizerの最小fixtureを準備 |
| P1 評価契約と責務分離 | core/facade/backend境界、Input/Report/Error/config、legacy移設、REQ/API/ARC改訂 | P0 | 新coreにgit2/pulldown/Candle依存なし。SLMなし起動。原文保持・span・未評価・schemaの契約試験が通る |
| P2 一次検出MVP | Sudachi adapter、全形態素features、少数の説明可能rules、profile、JSON出力と評価runner | P1 | OCR/ASR/LLM/formのテキスト例を同APIで評価。正常語・短文・固有名詞を含むbaseline。SLM呼出0、訂正文生成0 |
| P3 選択的SLM | tokenizer修復/互換bundle、judge、routing、有界worker、遅延ロード、失敗時保留 | P2 | route対象以外のSLM呼出0。実モデル日本語試験。context上限/timeout/invalid output/予算超過/同時実行を確認 |
| P4 校正・品質受入 | 分割済dataset、rule-only/SLM-only/cascade比較、calibration、domain別threshold、CPU bench | P2・P3 | 固定test集合でprecision/recall・呼出率・latency/RSSとcalibrationを報告。事前合意したSLOを満たす |
| P5 拡張・移行リリース | domain profile追加、fine-tune/adapter bundle試験、移行guide、legacy deprecation方針 | P4 | bundle差替え・互換性拒否・rollbackが可能。互換APIの利用例とfeature matrixが動く |

互換性方針: 0.1の公開APIを黙って別の意味に変更しない。新evaluate APIを追加して利用例を作り、legacy facadeを別crateへ移した後、0.2で明示的な破壊的変更と移行先を公開する。既存利用者数・互換要求が判明したらdeprecated shimを残す期間を決める。新デフォルトにGit/生成を残して分離完了としない。

旧REQ/ARC/DETAIL/API/TESTの001文書は履歴として保持し、新版またはsupersededの注記で新計画との優先順位を示す。旧タスクのcompletedを新機能のcompletedへ流用しない。

## 9. 品質評価・受入条件

評価データは自然な正常文、人工破損だけでなく実際のOCR/ASR誤り、助詞欠落、反復、否定反転、数値/単位不一致、流暢な意味矛盾、文脈不足を含む。方言・固有名詞・製品コード・箇条書き・短いform値・混在言語も正常/保留例として用意する。人手labelは三軸別に付け、判断不能・意見不一致を残す。

原文・話者・文書・domain単位でtrain/calibration/testを分け、同じ原文からの人工変形が跨がないようにする。校正にはSLMに送った例だけでなく、一次通過例・保留例を含める。運用分布の代表集合と意味誤りのstress集合を分けて報告する。

| 観点 | 測るもの / 受入判断 |
| --- | --- |
| 検出品質 | 軸別・issue別・用途別precision/recall、正常文誤警報率、重大誤り見逃し率、span適合 |
| gate | 真の誤りが一次で確定または二次候補になった割合、通過側の見逃し率、SLM document率/segment率、推論token総量 |
| 保留 | undetermined率・coverage、処理できたものだけの精度と全入力での精度を併記 |
| calibration | Brier/ECE・reliability plot、route別/domain別のズレ。heuristic段階では校正済と表示しない |
| CPU性能 | 辞書のみ/SLM cold/warm別p50/p95、queue含む全体latency、throughput、peak RSS、初期化時間 |
| 信頼性 | SLM故障をacceptableへ変換しない、低risk経路のmodel未ロード、同じ入力/設定で安定、原文不変 |
| 回帰 | 日本語byte span、結合文字/emoji、空文字とrequired field、長文/URL/小数、reference差替え、命令文を含む評価対象 |

数値SLOはP2 baseline後、CPU型番・RAM・thread数・入力長・domain・重大誤り定義を固定してP3開始前に決定する。例としてSLM率を20%以下に置く場合も、recallを犠牲にして達成しない。全件SLMは本番既定にはせず、オフライン比較用baselineとしてのみ実行する。SLM単体も正解oracleとして扱わない。

## 10. 今回実施した検証と限界

- 作業前のgit statusはclean。実装コードは変更していない。
- CargoはPATH外だったため、`C:\Users\Shion\.cargo\bin\cargo.exe`で実行した。
- `cargo fmt --all -- --check`: 成功（canonicalize警告あり）。初回読取りで疑った文分割の構文破損は再確認で否定し、監査指摘から除外した。
- `cargo metadata --offline --no-deps --format-version 1`: 成功。targetはlib、performance bench、build scriptのみ。test targetなし。
- `cargo test --offline --locked --all-features`: Sudachi v0.6.9のgit依存がローカル未取得のため、依存解決で失敗。コンパイル・テスト本体は未実行。
- 実辞書による精度・実モデル推論・CPU性能・CIの過去実行結果は未検証。静的監査での欠陥と実測結果を混同しない。
- Rust公式にも、tests配下のサブディレクトリ内ファイルはそれだけでは個別test crateとしてコンパイルされないと記載されている。[Test Organization](https://doc.rust-lang.org/book/ch11-03-test-organization.html)

## 11. 実装開始時に確定する事項

本計画は「日本語、Rust API、ローカル資産、まず一次検出MVP、SLMは任意」を既定とする。P2の実データ評価開始までに、最初の重点用途、許容誤警報/見逃し、対象CPU/RAM、評価文脈の提供方法を確定する。モデル銘柄・サイズ・固定threshold・adapter形式は、その後の互換性と品質計測に基づき選定する。
