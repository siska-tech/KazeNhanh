# KazeNhanh 開発ロードマップ

取り組むべき次のタスク : recognition risk再設計 R2（軽量risk baseline）→ R3（選択LM/判別器）

2026-10-04: [再設計案002](../docs/KZN-REDESIGN-PLAN-002.md)を作成。OCR/ASR共通recognition riskを主目的に変更し、naturalnessは補助、Qwen judgeは実験比較用とする。R0契約/保留とR1 source evidenceは実装済み、R2は統計evidence/ローカル資産を先行実装して進行中、R3/R4は未着手。誤認識検出品質の改善を意味しない。以下のP0〜P3は従来計画での実績・履歴として保持。

2026-10-04: [再設計監査・計画](../docs/KZN-REDESIGN-PLAN-001.md)を作成。[監査タスク](task-redesign-001-text-evaluation-audit.md)はcompleted、[移行タスク](task-redesign-002-text-evaluation-migration.md) / [Issue #2](https://github.com/siska-tech/KazeNhanh/issues/2)でP0完了。P1/P2も完了。P3は制御契約を実装して進行中、P4/P5は未着手。以下の9件・44件は旧Git/要約機能の履歴であり、新基盤の進捗や検証済み品質を示しません。

P0完了: 明示fake注入、実辞書・合成/学習済みGGUFのCPU推論、公式tokenizerの独立参照ID照合を検証。固定モデルセットアップと実モデルCIを追加。Windows/Linux QA・TSan・Criterion/Soakもremote成功。

## 旧Git・要約機能のタスク記録
完了タスク件数 / 総タスク件数 : 9 / 9
完了サブタスク件数 / 総サブタスク件数 : 44 / 44 （進捗率: 100%）

| No  | タスクID                             | 概要                                 | ステータス | 優先度 | サブタスク                                            | 依存関係             | 参照ドキュメント                          |
| --- | ------------------------------------ | ------------------------------------ | ---------- | ------ | ----------------------------------------------------- | -------------------- | ----------------------------------------- |
| 1   | task-core-001-kaze-nhanh-engine      | ファサードエンジンとエラー統合を実装 | completed  | high   | [一覧](task-core-001-kaze-nhanh-engine.md) (6/6)      | なし                 | KZN-API-SPEC-001, KZN-DETAIL-DESIGN-001   |
| 2   | task-foundation-001-nlp-service      | SudachiベースNLPサービス             | completed  | high   | [一覧](task-foundation-001-nlp-service.md) (5/5)      | task-core-001        | KZN-DETAIL-DESIGN-001, KZN-REQ-SPEC-001   |
| 3   | task-foundation-002-markdown-service | Markdown構造マッピング               | completed  | medium | [一覧](task-foundation-002-markdown-service.md) (5/5) | task-core-001        | KZN-DETAIL-DESIGN-001, KZN-REQ-SPEC-001   |
| 4   | task-foundation-003-git-service      | Git差分抽出サービス                  | completed  | high   | [一覧](task-foundation-003-git-service.md) (5/5)      | task-core-001        | KZN-DETAIL-DESIGN-001, KZN-REQ-SPEC-001   |
| 5   | task-inference-001-inference-engine  | candle推論エンジン実装               | completed  | high   | [一覧](task-inference-001-inference-engine.md) (6/6)  | task-core-001        | KZN-DETAIL-DESIGN-001, KZN-ARC-DESIGN-001 |
| 6   | task-pipeline-001-hybrid-summarizer  | HybridSummarizerとLexRank            | completed  | medium | [一覧](task-pipeline-001-hybrid-summarizer.md) (5/5)  | foundation/inference | KZN-DETAIL-DESIGN-001                     |
| 7   | task-pipeline-002-git-native-rag     | GitNativeRAGパイプライン             | completed  | high   | [一覧](task-pipeline-002-git-native-rag.md) (6/6)     | foundation/inference | KZN-DETAIL-DESIGN-001, KZN-REQ-SPEC-001   |
| 8   | task-testing-001-quality-assurance   | 総合テスト体制整備                   | completed  | medium | [一覧](task-testing-001-quality-assurance.md) (6/6)   | パイプライン完了後   | KZN-TEST-SPEC-001                         |
| 9   | task-demo-001-git-sample-repo        | デモ用Gitリポジトリ整備               | completed  | medium | [一覧](task-demo-001-git-sample-repo.md) (0/0)        | なし                 | README, demo/repo                        |

> 各タスクは設計書の要件を満たす実装および検証を対象とし、進行状況に応じてステータスと日付を更新すること。





P1完了: 独立coreの契約試験、新Sudachi・legacyの分離、0.2 feature境界を検証。[API仕様002](../docs/KZN-API-SPEC-002.md)。

2026-10-04: ユーザーの明示承認後にcore/Sudachi/legacyをCargo workspaceへ分離し、0.2へ切替。P1完了。default/minimalの依存境界とモデル不要起動、owned辞書・Mode・原文span・並行評価、旧APIの回帰を検証。[0.2移行ガイド](../docs/migration-0.2.md)。以前の承認待ち記録は解消済み。

2026-10-04: P2一次検出MVPを実装。profile・形態素features・説明可能rules、4用途共通APIとJSON runner。34 fixture全期待一致、正常19件の誤警報0、意味保留4件、SLM呼出0。[検出範囲](../docs/primary-detection.md)。

2026-10-04: P3の選択的二次worker・遅延load・共有予算・deadline・有界queue・厳格出力と保留を実装。優先はOCR/ASR。実モデル日本語judge/tokenizer bundle/CPU SLOは未完了。[制御契約](../docs/secondary-judging.md)。

2026-10-04: P3実験用Qwen自然さadapter・固定資産setup・独立token ID照合・CPU smokeを追加。P3全体の品質/意味adapter/SLO受入は未完了。[adapter](../docs/qwen-judge.md)。

2026-10-04: 提供PP-OCRv6 medium実認識5件を保存・投入。画像転記はユーザー確認済み。不一致3件すべてを現gate/実験用Qwenが検出できず、P3品質未受入を維持。[実測](../docs/ocr-samples-user-001.md)。

2026-10-04: 追加OCR 2画像/10件を原文・confidence・document ID付きで保存（計3画像/15件）。「体系キープ」「10kgやせる」の転記はユーザー確認済み、他8件は画像転記未確認。今回の転記差5件（確認済み1件）すべてが一次gateを通過。runner既定はモデル不要、SLM呼出/forward=0、Qwen比較は明示opt-inへ変更。P3品質未受入を維持。[追加データと観察](../docs/ocr-samples-user-002-003.md)。

2026-10-04: [R0 API](../docs/recognition-api.md)を追加。別schema・risk/null・evidence欠測・review/undetermined、モデル不要facade/CLI、提供OCR15件の保持/保留を検証。低リスク受理とrisk estimatorは未実装。

2026-10-04: [R1 source evidence](../docs/recognition-source-evidence.md)を追加。型付きconfidence/N-best、source別adapter/profile、欠測とbounded alignment、report v2、15件runnerを検証。全件保留で、risk推定・ASR実品質は未評価。

2026-10-04: [R2統計evidence](../docs/recognition-statistics.md)を先行実装。文字/語n-gram、domain・辞書資産照合、clean corpus builder、hash検証と1回解析を追加。提供OCR15件は全件保留。検出/fusion・独立実OCR/ASR品質比較が残り、R2はprogress。

2026-10-04: [small/core/full辞書比較](../docs/sudachi-dictionary-matrix.md)を追加。同版・Mode CでOCR15件と人工hard-cleanを観察し、統計assetを辞書別に生成。分割差はあるがOCR判定は全件保留のまま。実OCR/ASR・fusion/CPU品質の受入は継続。

2026-10-05: [R2候補review baseline](../docs/recognition-candidate-review.md)を追加。raw候補不一致を明示opt-inでreviewへ送り、rank/spanを保存。人工12例を3辞書で比較。候補の正解性・誤り確率は未推定で、R2実品質受入は継続。

2026-10-05: [R2オフライン品質集計](../docs/recognition-quality.md)を追加。確認済み転記だけでreview精度/再現率と低リスク受理率を計算し、保留を検出成功に数えない。3辞書matrixへ統合。独立実OCR/ASR評価とfusionは未完了。

2026-10-05: [評価データsplit監査と公開サンプル取得](../docs/recognition-datasets.md)を追加。人工fixtureと実測を区別し、実OCR/ASR集合の採用・融合判定は引き続きR2の残作業。

2026-10-05: [軽量統計review baseline](../docs/recognition-sparse-review.md)を実装。既定は保留を維持し、明示有効時のみ三条件共起をreview。候補不一致とのOR統合、risk=null。独立評価/fusionの受入は継続。

2026-10-05: [POS bigram特徴](../docs/recognition-pos-statistics.md)を追加。既存解析を共有し、欠測tokenを跨がず、学習POSなしは未評価。3辞書比較をv1とは別出力へ保存。R2判定・品質受入は継続。

R2文字種観測（2026-10-05）: 固定範囲のscalar件数・隣接遷移・有界spanを実装。判定条件/source policy/fusionと品質受入は残作業。[仕様](../docs/recognition-string-features.md)。

R2条件比較（2026-10-05）: POS/文字種と既存統計条件の5比較・欠測別集計を実装。3辞書の観察では単純条件の採用根拠なし。source policy/fusion・品質受入は継続。[詳細](../docs/recognition-feature-ablation.md)。

R2 source confidence契約（2026-10-05）: adapter側の厳格binding・閾値比較・適用不能を実装。core policy接続/fusionと閾値受入は未完了。[仕様](../docs/recognition-confidence-review.md)。

2026-10-06: kzn-ocr-synth-1kの1,000件を登録、split監査とローカル推論/参照分離を完了。null confidence対応とtrain限定統計asset・3辞書development評価が次段階。[詳細](../docs/ocr-synth-1k.md)。

2026-10-06: OCR 1kのnull confidence対応、train限定統計assetと3辞書development比較を実装。統計頻度のJSON往復不整合も回帰テスト付きで修正。calibration/testの推論なし。[結果](../docs/ocr-synth-1k.md)。

2026-10-06: confidence ruleのevaluate/combineと別namespaceの統合reportを追加。base保持・JSON再検証・適用不能理由を実装。source定義/品質runner・閾値受入は残作業。[仕様](../docs/recognition-confidence-review.md)。

2026-10-06: source統合reportのoffline品質集計を実装。最終/base判断を分離、旧形式互換、rule ID衝突拒否。実データsource定義と閾値採用は残作業。

2026-10-06: 生成コード/hashに基づくOCR source定義とtrainラベル非使用の下位10%比較を完了。3辞書ともconfidenceは不一致9件・一致0件、sparse ORの追加検出0。校正・test受入は未実施。

2026-10-06: confidence/text/integratedの固定logistic比較を実装。train限定標準化・学習、development評価、重複/identity検査を追加。統合は誤警報64/66で未採用。次はout-of-fold統計特徴の検討。[結果](../docs/recognition-fusion-baseline.md)。
