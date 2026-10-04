# Recognition統計evidence（R2先行実装）

2026-10-04。R2はprogress。今回実装したのはモデル不要の特徴抽出とローカル統計資産。統計を用いた検出器・fusion・校正済risk・低リスクpolicyは未実装。[計画002](KZN-REDESIGN-PLAN-002.md)のR2完了条件は独立した実OCR/ASR集合での品質比較も含む。

## APIと出力

`StatisticsArtifact::fit(metadata, documents)`でclean corpusの文字列と解析済み形態素から疎な頻度表を構築する。`LightweightStatistics::new(artifact)`で資産を検証し、`RecognitionEngine::with_statistics(Arc<LightweightStatistics>)`で明示接続する。既定では統計資産をロードしない。RecognitionInput.domainを宣言し、資産のdomainと完全一致する場合だけ使用する。source profileとは別の契約で、同じdomainでも認識品質が検証済みとはしない。

- 文字bigram: 原文Unicode scalar列の隣接2文字。空白・記号・改行も保持し、segmentを跨がない。
- 語unigram/bigram: 全形態素のdictionary_form（空ならsurface）。助詞・助動詞・OOVも含む。byte spanは原文のsurface位置。
- 各系列のunit_count、unseen_count、unseen_fraction、未観測位置。分母が0ならfraction=null。観測0%と未評価を区別する。
- oov_countと計算法・asset/corpus ID・corpus SHA256・domain・学習segment数。

形態素解析は一次ルールと統計で1回の結果を共有する。LexicalStatistics familyをobservedにし、その他のevidenceとprimary findingsを保持する。未知/欠落domain、辞書/settings/解析mode等のidentity不一致、統計入力上限超過はunsupportedと理由を返す。資産未接続ならunsupported / lexical_statistics_not_configured。

値は**コーパス内の頻度観察**であり、未校正anomaly indexでも誤認識確率でもない。未観測率が高いだけでreviewにしない。低頻度・OOV・筆者の誤字・口語・固有名詞・型番を認識誤りに変換しない。統計なしの判断を維持し、既存の一次warningはreview、無警告はundetermined。risk=null、naturalness=null、SLM呼出0。全件保留を検出成功と呼ばない。

## 資産と境界

資産schemaはkzn.statistics.v1。未知schema/field、空のID/domain/license/hash/解析identity、重複identity、0件corpus、不正頻度・総数・非canonical pair、サイズ上限超過を拒否する。解析identityは辞書・settings hashとbackend/modeを含め完全一致する。既存report schema kzn.recognition.v2を維持し、従来unsupportedだったLexicalStatisticsのobserved payloadを定義した。report.validateはpayloadのID/domain、分母・率・件数、省略件数・原文span、形態素数との一致も確認する。

Coreはファイル取得やhash計算をしない。外部資産の真正性は呼出側で確認する。提供runnerは**指定asset SHA256**を読み込んだbytesと照合してから検証/使用する。reportのprovenance.sha256はファイルhash未提供のためnull、観測payloadにはcorpus hashを保持する。検証summaryにはasset file hashも記録する。hash一致は資産同一性の確認で、コーパス品質・適用性能の証明ではない。

資産は3表合計100,000 entries、key 4,096 bytes、fitは10,000 segments・各65,536 bytesに制限。builder CLIは8 MiB/4,096 segments、runnerのasset入力も8 MiBに制限。未観測位置は系列ごと最大64件、省略数と全件カウントは保持する。比較はraw文字を用いる。文字種遷移/POS統計、条件付きsurprisal、融合重み、閾値は未実装。core依存はserde/serde_json/thiserrorのまま。

## オフライン利用

以下は機能試験用。8件の人工clean corpusはCC0-1.0で、この実装用に作成した[契約fixture](../tests/fixtures/statistics/README.md)。代表日本語資産・実ASRデータではない。ユーザーOCR15件の転記やラベルを学習へ投入していない。

```powershell
cargo run --locked --offline --example statistics_asset -- tests/fixtures/statistics/clean-contract.jsonl authored-contract-v1 contract_fixture CC0-1.0 target/statistics/contract.json
$statisticsHash = (Get-FileHash -Algorithm SHA256 target/statistics/contract.json).Hash.ToLowerInvariant()
cargo run --locked --offline --example recognition_samples -- evaluation/ppocrv6-medium-user-001.jsonl target/statistics/ocr.json target/statistics/contract.json $statisticsHash contract_fixture
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev/verify-statistics.ps1 -Offline
```

実データへのdomain指定は人工fixtureを用いた機能観察で、適用domainとして受入済みとはしない。本番は独自clean corpusと解析資産を固定して検証する。corpusはid/document_id/textのみ受理し、gold/reference/confidence等の未知fieldを拒否。OCR runnerも参照/期待labelを推論入力へ渡さない。builderはネットワーク・モデルを使わず、生成の再現性は同一入力bytes/辞書/settings/modeで検証する。

verify.ps1へ統合済み。統計生成・hash拒否をCIでも確認する。ユーザーOCR15件の観察レポートはローカルのtarget/statistics-verificationに保存し、CIではこの観察を省略する。OCR統計レポートのCIアップロード追加は自動承認レビューが拒否したため取り下げた。core/実Sudachi試験ではUnicode span・空入力・event上限・domain/辞書版不一致、1回解析、固有名詞/口語/「体系キープ」の保留、既存warning保持とpayload改ざん拒否を確認する。

## 観察とR2残作業

2026-10-04ローカル観察: 提供OCR15件の統計を抽出。review=0、undetermined=15、low_risk=0、risk推定=0、SLM呼出=0。人工8文からの未観測率は誤り検出品質の指標ではない。独立した実OCR/ASRでのprecision/recallやCPU SLOは未評価。

次は出所・転記規約・splitを固定した実認識対と難しいclean例を整備し、confidence-only/text-only/source-onlyと統合baselineを比較する。語彙/文字列/語列異常ルール、文字種/POS統計、小さなfusionを評価し、reviewと単なる保留の実数を報告する。現在の15件へ重み・閾値を合わせ込まない。R2を完了とせず、R3/R4の選択LM・校正・低リスク受理も後続とする。
