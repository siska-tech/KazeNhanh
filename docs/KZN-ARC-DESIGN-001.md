# **KazeNhanh アーキテクチャ設計書**

**文書ID:** KZN-ARC-DESIGN-001
**バージョン:** 1.0
**ステータス:** 確定 (Confirmed)
**作成者:** Shion Watanabe
**日付:** 2025/11/7

---

## **序文: アーキテクチャの基本方針**

本アーキテクチャ設計書は、要件定義書（REQ-SPEC-001）に定義されたライブラリ「KazeNhanh」を実現するための技術的な青写真である。

本設計のすべての決定は、要件定義書セクション 2.0 で定義された3つの揺るぎない柱に基づいている。

1. **ローカルファースト & CPUオンリー (REQ 2.1):** アーキテクチャレベルで外部ネットワークへの依存（std::net等）を排除し、AI推論（FR 3.3.2）はローカルに存在する GGUF 形式の量子化モデル（REQ 5.6）でのみ実行される。これにより、NFR 2.1（オフライン）および NFR 2.2（データ非送信）を保証する。  
2. **日本語ネイティブ (REQ 2.2):** sudachi.rs 1 と saku 2 をNLP基盤（FR 1）の中核に据える。統計アルゴリズム（FR 3.3.1: LexRank）は、これらのクレートによって形態素レベル・文レベルで解析された日本語テキストを前提として動作する。  
3. **Gitネイティブ (REQ 2.3):** 従来のベクトルデータベース（VDB）ベースのRAGを意図的に採用しない。「コンテキストとしての差分 (Diff-as-Context)」アプローチを中核とし、git2-rs 3 が抽出した「変更箇所（diff）」と pulldown-cmark 4 が解析した「ドキュメント構造」の相関（FR 4.4）を、AIへのリッチな入力コンテキストとして利用する。

本アーキテクチャを定義する最も重要な制約は、NFR 4.3（統合と配布）と REQ 5.0（制約条件）の組み合わせである。特に NFR 4.3.3（導入容易性）は、Tauri アプリの**エンドユーザー**が Python 環境のセットアップやモデルのダウンロードといった一切の作業から解放されることを要求している。

この要求は、Tauri アプリがAIランタイムとモデル自体を単一バイナリ（NFR 4.3.2）にバンドルする必要があることを技術的に意味する。この制約を満たすため、アーキテクチャは必然的に「Pure Rust」(REQ 5.1) の技術スタックに依存しなければならない。Python ベースのフレームワークを排除し、Rust ネイティブのMLフレームワーク candle 5 を採用すること（REQ 5.2）は、この「ゼロ・ランタイム依存」を実現するための技術的帰結である。

同様に、git2-rs 6 の採用は、libgit2 を静的にバンドル（vendored）する機能を提供し、エンドユーザーのPCに git がインストールされている必要性すら排除する。本設計書は、この「ゼロ・ランタイム依存」を最優先事項として構築される。

## **1\. システム構成図（コンポーネント図）**

要件定義書（REQ-SPEC-001）に示された全要件を満たし、かつ高い保守性と拡張性を担保するため、ライブラリ全体を「関心の分離 (Separation of Concerns)」の原則に基づき、4つの独立した内部レイヤー（Crates）として構成する。

### **1.1 アーキテクチャ全体像（レイヤー構造図）**

ライブラリの依存関係は、厳格なトップダウン（上位レイヤーから下位レイヤーへの一方的な依存）として定義される。下位レイヤーは、自身がどのレイヤーから呼び出されているかを認識しない。

\+-------------------------------------------------------------+

| |  
| (NFR 4.3.1: Tauri Integration) |  
\+-------------------------------------------------------------+  
|  
                           v (Public API)  
\+-------------------------------------------------------------+

| \[L1\] kaze\_facade (ファサード・レイヤー) |  
| 責務: \- 公開APIの提供 (Facade パターン) |  
| \- 内部エラーの集約 (NFR 4.4.1: KazeError) |  
\+-------------------------------------------------------------+  
|  
                           v (Internal Crates)  
\+-------------------------------------------------------------+

| \[L2\] kaze\_pipeline (パイプライン・レイヤー) |  
| 責務: \- ビジネスロジックの実行とオーケストレーション |  
| \- HybridSummarizer (FR 3\) |  
| \- GitNativeRAG (FR 4\) |  
\+-------------------------------------------------------------+

| |  
                v                            v  
\+-----------------------------+ \+-----------------------------+

| \[L3\] kaze\_inference | | \[L4\] kaze\_foundation |  
| (推論エンジン・レイヤー) | | (基盤サービス・レイヤー) |  
| 責務: \- SLM推論 (FR 3.3.2) | | 責務: \- 必須スタックの隠蔽 |  
| \- candle (REQ 5.2) | | \- NlpService (FR 1\) |  
| \- CPU-Only (NFR 1.1) | | \- MarkdownService (FR 2)|  
| \- Singleton (NFR 1.2) | | \- GitService (FR 4.1) |  
\+-----------------------------+ \+-----------------------------+

### **1.2 レイヤー定義とコンポーネントの責務**

#### **L1: kaze\_facade (ファサード・レイヤー)**

* 責務 (Responsibility):  
  KazeNhanh ライブラリの唯一の公開インターフェース（Public API）を提供する。Tauri バックエンド（NFR 4.3.1）や他の Rust アプリケーションが直接利用する最上位レイヤーである。  
* **機能:**  
  * **Facade パターン:** L2（パイプライン）の複雑なロジック実行や L3（推論エンジン）のモデルロードをカプセル化し、summarize(text: \&str) \-\> Result\<String, KazeError\> や generate\_git\_report(path: \&Path, days: u32) \-\> Result\<String, KazeError\> のような単純な関数として提供する。  
  * **エラーハンドリング (NFR 4.4.1):** ライブラリ全体のエラーハンドリングの終端点として機能する。thiserror クレートを用い、L2, L3, L4 から伝播されるすべての内部エラー（git2::Error, candle\_core::Error, ParseError 等）を、統一された公開エラー型 KazeError にラップし、Result 型として呼び出し元に返却する。

#### **L2: kaze\_pipeline (パイプライン・レイヤー)**

* 責務:  
  機能要件（FR）で定義された主要なビジネスロジックと、L3/L4 のサービスを組み合わせるオーケストレーションを実行する。  
* **主要コンポーネント:**  
  * HybridSummarizer: FR 3（ハイブリッド要約）の「Drip and Stir」パイプラインを実装する。L4 の NlpService から日本語解析結果を取得し、内部の統計アルゴリズム（FR 3.3.1: LexRank）を実行（Drip）する。その結果（重要文）を L3 の InferenceEngine に渡し、自然言語の段落を合成（Stir）させる。  
  * GitNativeRAG: FR 4（GitネイティブRAG）の5ステップ（FR 4.1〜4.5）を実行する。L4 の GitService と MarkdownService を呼び出し、差分（diff）とドキュメント構造を「相関」（FR 4.4）させる。その相関結果を L3 の InferenceEngine に渡し、レポートを生成（FR 4.5）させる。

#### **L3: kaze\_inference (推論エンジン・レイヤー)**

* 責務:  
  FR 3.3.2（合成エンジン）の責務を単独で担う。AIモデルのロードと実行に関するすべての複雑性をカプセル化する。  
* **機能:**  
  * candle 5 を利用し、指定された GGUF モデルファイル（REQ 5.6）をロードする。  
  * ビルド構成において cuda や metal などの GPU 関連フィーチャーを意図的に無効化し、CPU バックエンド（デフォルトまたは mkl）のみを利用する。これにより NFR 1.1（CPUオンリー）を強制する。  
  * fn synthesize(context: String) \-\> Result\<String, KazeError\> という単一のインターフェースを L2 に提供する。  
* 設計上の重要点 (Singleton の強制):  
  NFR 1.2（速度）および NFR 1.3（リソース消費）の要件を満たすため、このレイヤーは Singleton パターンを採用しなければならない。GGUF 形式の SLM は、ファイルサイズが数 GB に達し、CPU メモリへのロードに数秒を要する場合がある。FR 3（要約）と FR 4（RAG）が呼び出されるたびにモデルをロードする設計は、NFR 1.2 で要求される「現実的な速度」を著しく損なう。  
  これを解決するため、once\_cell::sync::Lazy または lazy\_static 8 を用い、ロード済みの CandleEngine インスタンスをグローバルな静的（Singleton）インスタンスとして保持する。これにより、Tauri アプリケーションのプロセス起動後の初回アクセス時のみロードが発生し、2回目以降の synthesize 呼び出しは、ロード済みのモデルを即座に再利用して実行される。これは、FR 3.3.2 で要求された「モジュール性」（FR 3 と FR 4 からの再利用）を実現する上でも不可欠な設計である。

#### **L4: kaze\_foundation (基盤サービス・レイヤー)**

* 責務:  
  必須技術スタック（REQ 5.0）をラップし、安定した Pure Rust インターフェースを L2 に提供する。FFI バインディング（git2-rs）や複雑な外部クレート（sudachi.rs）の技術的詳細を隠蔽する。  
* **主要コンポーネント:**  
  * NlpService: FR 1（日本語NLP基盤）を担当。saku 2 による文分割 (FR 1.1) と、sudachi.rs 1 による形態素解析 (FR 1.2) を提供する。  
  * MarkdownService: FR 2（Markdown解析基盤）を担当。pulldown-cmark 4 を利用し、ドキュメント構造 (FR 2.1) と、FR 2.2 で要求される「行番号マッピング」を提供する（詳細は 2.3 節）。  
  * GitService: FR 4.1 および FR 4.2 を担当。git2-rs 3 を利用し、指定されたリポジトリパスと期間に基づき、変更された Markdown ファイルの diff を解析し、追加された行番号と内容を抽出する。

## **2\. モジュール間の関係**

主要なユースケース（FR 3, FR 4）におけるレイヤー間の連携、データフロー、および重要ロジックを定義する。

### **2.1 データフロー: FR 3 (ハイブリッド要約 \- "Drip and Stir")**

L2:HybridSummarizer が実行する2段階のパイプラインのデータフローは以下の通りである。

**ステージ1: 抽出 (Drip) \- L2 と L4 の連携**

1. L1:Facade が summarize を呼び出し、L2:HybridSummarizer に raw\_text: String を渡す。  
2. L2 は L4:NlpService::segment(raw\_text) を呼び出す。saku 2 がテキストを文のリスト Vec\<String\> に分割する (FR 1.1)。  
3. L2 は L4:NlpService::tokenize\_sentences(sentences) を呼び出す。sudachi.rs 1 が各文を形態素解析し、Vec\<Sentence\>（トークン情報を含む構造体）を返す (FR 1.2)。  
4. L2:HybridSummarizer は、内部に実装された LexRankEngine（FR 3.3.1）を実行する。このエンジンは、Vec\<Sentence\> をノードとし、文間の類似度（トークンベースで計算）をエッジとするグラフを構築し、PageRank 9 を計算する（詳細は 4.2 節）。  
5. LexRankEngine は、ランク上位の文（例：5〜10文）のリスト Vec\<Sentence\> を L2 に返す。

ステージ2: 合成 (Stir) \- L2 と L3 の連携  
6\. L2 は、抽出されたランク上位の文を結合し、SLM へのコンテキスト（プロンプト）を作成する。（例: 「以下の文を自然な日本語の段落として再構成してください: \[文1\]\[文2\]...」）  
7\. L2 は L3:InferenceEngine::synthesize(prompt) を呼び出す。L3 は（1.2 節で定義された）Singleton の candle 5 インスタンスを使用する。  
8\. L3 は合成された要約パラグラフ summary\_paragraph: String を L2 に返す (FR 3.3.2)。  
9\. L2 は L1 を経由し、最終的な要約を呼び出し元に返す。

### **2.2 シーケンス: FR 4 (GitネイティブRAG)**

本ライブラリのキラー機能（FR 4）である Git ネイティブ RAG の5ステップパイプラインにおける、モジュール間のシーケンスは以下の通りである。

*(シーケンス図のテキスト表現)*

**FR 4.1 (差分の検出) & FR 4.2 (変更箇所の特定):**

1. TauriApp \-\> L1:Facade::generate\_git\_report(path, days)  
2. L1:Facade \-\> L2:GitNativeRAG::execute(path, days)  
3. L2:GitNativeRAG \-\> L4:GitService::get\_markdown\_diffs(path, days)  
   * L4:GitService は内部で git2-rs 6 を使用し、指定期間内のコミットをスキャンし、.md ファイルの diff を解析する。  
4. L4:GitService \-\> L2:GitNativeRAG  
   * 戻り値: Vec\<FileDiff\>  
   * FileDiff は { path: PathBuf, added\_lines: Vec\<(usize, String)\> }（行番号と追加行テキストのタプル）のリストを含む構造体である。

FR 4.3 (文脈の特定) & FR 4.4 (相関):  
5\. L2:GitNativeRAG は Vec\<FileDiff\> のリストをイテレートする。  
6\. foreach diff in Vec\<FileDiff\>:  
1\. L2 \-\> L4:MarkdownService::get\_structure\_map(diff.path)  
\* L4:MarkdownService は、指定されたファイルの Markdown 構造を解析する（詳細は 2.3 節）。  
2\. L4:MarkdownService \-\> L2  
\* 戻り値: structure\_map: HashMap\<Range\<usize\>, String\>  
\* 例: {(20..30) \=\> "\#\# 完了したタスク", (31..50) \=\> "\#\#\# 認証フロー"}  
3\. L2:GitNativeRAG が 相関 (Correlation) ロジックを実行する (FR 4.4)。  
\* diff.added\_lines（例: (25, "+ Task C が完了")）をイテレートする。  
\* L2 は「行番号 25 は structure\_map のキー (20..30) の範囲内である」ことを特定する。  
\* L2 は、この変更が "\#\# 完了したタスク" セクションで発生したという事実（ContextualChange オブジェクト）を保存する。  
FR 4.5 (レポート生成):  
7\. L2:GitNativeRAG は、すべてのファイルから集約した Vec\<ContextualChange\> のリストを構築する。  
8\. L2 は、このリストを FR 3.4.5 で要求された形式に準拠した、人間が読める形式のプロンプトにフォーマットする。（例: 「以下の変更履歴を基に、今週の進捗レポートを作成してください: \\n \- project-A/tasks.md の \#\# 完了したタスク セクションに変更: \+ Task C が完了 \\n...」）  
9\. L2 \-\> L3:InferenceEngine::synthesize(prompt)  
\* \[重要\] ここで、FR 3.3.2 の合成エンジン（L3）が 再利用 される。  
10\. L3:InferenceEngine \-\> L2 (戻り値: report\_text: String。例: 「今週の進捗： project-A/tasks.md の \#\# 完了セクションに1つのタスクが追加されました。」）  
11\. L2 \-\> L1:Facade \-\> TauriApp (最終的なレポート)  
この設計（L3 の再利用）は、FR 3.3.2（モジュール性）の要件を満たすと同時に、NFR 1.3（リソース消費）の要件にも準拠する。AI エンジンの責務は「テキストの合成」のみに限定され、タスク固有のロジック（プロンプト作成）は L2 が担当する。

### **2.3 FR 2.2 のための重要設計: 行番号マッピング**

FR 4.4（相関）の核となる L4:MarkdownService の行番号マッピング（FR 2.2）は、以下の通り実装される。

* **要件:** FR 2.2 は、Markdown の論理構造（見出し）と、それがファイル内の物理的な行番号のどの範囲（開始行・終了行）に対応するかをマッピングすることを要求している。  
* **実装方針 (L4:MarkdownService):**  
  1. pulldown-cmark 4 は、Parser::new\_ext コンストラクタにおいて Options::ENABLE\_SOURCE\_POSITION を有効化することで、各イベント（Event）が発生したソースコード上の行番号（Range）を公開する。  
  2. MarkdownService は、Parser から得られるイベントストリームをイテレートする。  
  3. Event::Start(Tag::Heading { level,.. }) を検知した際、そのイベントの開始行番号（start\_line）と見出しテキストを内部のスタックにプッシュする。  
  4. 次に、同じかそれ以下のレベルの Event::Start(Tag::Heading {.. }) またはドキュメントの終端（Event::End）を検知した際、そのイベントの直前の行番号を end\_line として取得する。  
  5. スタックから直近の見出しをポップし、HashMap\<Range\<usize\>, String\> に (start\_line..end\_line) \-\> "\#\# 見出しテキスト" として保存する。  
* 正当性:  
  このアプローチは、pulldown-cmark 4 の「pull parser」としての特性（低メモリ消費）を最大限に活用しつつ、ドキュメント全体を AST（抽象構文木）としてメモリにロードすることなく、FR 2.2 の厳密な要件（行番号マッピング）を満たすことができる。

## **3\. 設計原則・デザインパターン**

アーキテクチャの堅牢性、安全性、保守性を担保するため、REQ 2.0 のコア思想を技術的な原則に落とし込み、適切なデザインパターンを適用する。

### **3.1 コア設計思想のアーキテクチャへの適用**

* **ローカルファースト (REQ 2.1.1) \-\> Offline-by-Design（オフライン・バイ・デザイン）原則:**  
  * **適用:** L3:InferenceEngine は、モデルをロードするために外部 URL ではなく、ローカルのファイルパス（PathBuf）のみを受け付ける API を強制する。L4:Foundation レイヤーは、reqwest や hyper といった外部 HTTP 通信ライブラリへの依存を一切持たない。cargo-deny 等の CI ツールを用い、std::net モジュールの利用をライブラリ全体で禁止する。  
  * **効果:** これにより NFR 2.1（オフライン）および NFR 2.2（データ非送信）をアーキテクチャレベルで保証する。  
* **CPUオンリー (REQ 2.1.2) \-\> Zero-GPU Assumption（GPUゼロ前提）原則:**  
  * **適用:** L3:InferenceEngine が依存する candle 5 のビルド構成（Cargo.toml）において、cuda / metal / cudnn フィーチャーを意図的に disabled にする。AI 処理（FR 3.3.2）は、REQ 5.6 で指定された量子化モデル（GGUF）の CPU 推論に最適化される。  
  * **効果:** NFR 1.1（GPU非依存）をビルドレベルで保証する。  
* **Gitネイティブ (REQ 2.3) \-\> Diff-as-Context（コンテキストとしての差分）原則:**  
  * **適用:** L2:GitNativeRAG は、従来の RAG（ベクトルDBとセマンティックサーチ）をあえて排除する。代わりに、L4:GitService と L4:MarkdownService が生成する「変更箇所とセクションのリスト」（FR 4.4 の相関結果）という構造化された「メタデータ」を、SLM へのリッチなコンテキストとして利用する。  
  * **効果:** ライブラリの独自性（REQ 2.3.2）を確保し、ベクトル化処理のコスト（CPU負荷）を回避する。

### **3.2 主要デザインパターン**

* **Facade パターン:**  
  * **適用:** L1:kaze\_facade  
  * **理由:** L2 以下の複雑なレイヤー構造、candle のモデルロード（L3）、git2-rs の Diff 解析（L4）といった技術的詳細を、Tauri 開発者（NFR 4.3.1）から完全に隠蔽する。  
* **Singleton パターン (via once\_cell::sync::Lazy):**  
  * **適用:** L3:kaze\_inference::Engine  
  * **理由:** 2.1 節および 2.2 節で示された通り、FR 3 と FR 4 の両方が L3 を再利用する。SLM モデル（GGUF）のロードコスト（NFR 1.3）をプロセス起動後の初回アクセス時に限定し、2回目以降の synthesize 呼び出しを瞬時（NFR 1.2）に実行するため 8。  
* **Strategy パターン:**  
  * **適用:** L2:HybridSummarizer（FR 3.3.1）  
  * **理由:** 「重要文抽出アルゴリズム」をトレイト（例: trait ExtractiveEngine）として定義する。初期実装として LexRankStrategy を提供するが、将来的に TextRankStrategy や他のアルゴリズム（例：TF-IDF 11）に切り替える、またはユーザーが選択できるようにする拡張性を確保する。  
* **Builder パターン:**  
  * **適用:** L2:GitNativeRAG (FR 4\) の起動ロジック  
  * **理由:** GitRAGBuilder::new(path).days(7).ignore\_files(&).build().run() のように、将来的な拡張（例：無視するファイルパターン、コミット著者の指定）に対して柔軟な API を提供するため。  
* **エラーハンドリング (NFR 4.4.1):**  
  * **適用:** ライブラリ全体  
  * **理由:** Result\<T, KazeError\> を一貫して使用する。thiserror を利用して KazeError (public enum) を定義し、\#\[from\] git2::Error, \#\[from\] candle\_core::Error, \#\[from\] std::io::Error などのディレクティブを用い、下位レイヤー（L2, L3, L4）からの内部エラーを L1:Facade で自動的にラップし、集約する。

## **4\. 技術選定**

REQ-DRAFT-001 の制約条件（Section 5.0, 6.0）は、本アーキテクチャの根幹である。ここでは必須スタックの妥当性を再確認し、未確定要素（LexRank）の実装方針を決定する。

### **4.1 必須スタックの確定と正当性（REQ 6.0）**

REQ 6.0 で指定された技術スタックは、本アーキテクチャの非機能要件、特に NFR 4.3.2（単一バイナリ）と NFR 1.1（CPUオンリー）を達成するために必須であり、その選定は妥当である。

| カテゴリ | コンポーネント (Crate) | 役割 (FR) | 正当性 (NFR) および 参照 |
| :---- | :---- | :---- | :---- |
| **AI/ML推論** | candle-core | FR 3.3.2 | **NFR 4.3.3 (Python不要):** Pythonランタイムへの依存を排除する Pure Rust のMLフレームワークであるため 5。 |
| **AIモデル形式** | GGUF | REQ 5.6 | **NFR 1.1 (CPUオンリー):** candle がサポートする、CPU での実行に最適化された量子化モデル形式であるため。 |
| **Git操作** | git2-rs | FR 4.1, 4.2 | **NFR 4.3.2 (単一バイナリ):** libgit2 12 を静的リンク（vendored）する機能 6 を提供し、エンドユーザーの環境依存を排除するため 3。 |
| **Markdown解析** | pulldown-cmark | FR 2.1, 2.2 | **FR 2.2 (行番号):** 高速な Pure Rust パーサーであり、Options::ENABLE\_SOURCE\_POSITION による行番号マッピングをサポートするため 4。 |
| **NLP (形態素解析)** | sudachi.rs | FR 1.2, FR 3.3.1 | **REQ 2.2 (日本語):** 高品質な Pure Rust の日本語形態素解析器であり、辞書の同梱も可能であるため 1。 |
| **NLP (文分割)** | saku | FR 1.1, FR 3.3.1 | **REQ 2.2 (日本語):** 軽量で依存関係のない、ルールベースの Pure Rust 日本語文分割ライブラリであるため 2。 |

### **4.2 不確定要素の選定: FR 3.3.1 (LexRank / TextRank) の実装**

* 課題:  
  要件は FR 3.3.1 において、LexRank または TextRank（グラフベースの要約アルゴリズム）の実装を要求している。  
* 調査結果:  
  crates.io における Rust エコシステムの調査 10 の結果、以下の事実が判明した。  
  1. lexorank 15 という名前のクレートは存在するが、これは Atlassian Jira のランキングシステムであり、テキスト要約とは無関係である。  
  2. keyword\_extraction 16 や jieba-rs 17 は TextRank 機能を備えているが、jieba-rs は中国語に特化している。  
  3. pagerank\_rs 9 や simple-pagerank 18 など、汎用的な PageRank アルゴリズムの実装は存在する。  
  4. LexRank 19 および TextRank 20 は、本質的には PageRank アルゴリズム 9 を、文間の類似度（LexRank）または単語の共起（TextRank）に基づいて構築されたグラフに適用したものである 10。  
  5. REQ 2.2（日本語ネイティブ）の要件を満たすためには、sudachi.rs 1 が生成した形態素（トークン）を基に、文間の類似度を計算する必要がある。  
* 設計決定:  
  自前実装 (In-House Implementation) を採用する。  
* **実装方針 (L2: HybridSummarizer 内部):**  
  1. L2 は L4:NlpService を使用し、入力テキストを「文のリスト（グラフのノード）」と「各文の形態素（名詞・動詞）リスト」に変換する。  
  2. 文間の類似度（グラフのエッジの重み）を計算する。この類似度は、sudachi.rs によって得られたトークンリスト間の TF-IDF またはコサイン類似度として実装する。  
  3. この類似度行列（例：ndarray::Array2\<f64\>）を構築する 10。  
  4. この行列に対し、simple-pagerank 18 のような軽量な PageRank クレートを利用するか、あるいは 10 に示されているような反復計算ロジック（パワー法）を L2 内部に直接実装する。  
* 正当性:  
  このアプローチは、外部の巨大な NLP ライブラリや特定の言語（例：中国語 17）に特化したクレートへの依存を避ける。REQ 2.2（sudachi.rs による日本語理解）と REQ 5.1（Pure Rust）の制約を完全に満たしながら、FR 3.3.1 の要求機能（LexRank）を実現する、最も現実的かつ制約に準拠した解決策である。

#### **引用文献**

1. WorksApplications/sudachi.rs: Sudachi in Rust and new generation of SudachiPy \- GitHub, 11月 7, 2025にアクセス、 [https://github.com/WorksApplications/sudachi.rs](https://github.com/WorksApplications/sudachi.rs)  
2. saku \- crates.io: Rust Package Registry, 11月 7, 2025にアクセス、 [https://crates.io/crates/saku](https://crates.io/crates/saku)  
3. git2 \- Rust \- Docs.rs, 11月 7, 2025にアクセス、 [https://docs.rs/git2](https://docs.rs/git2)  
4. pulldown-cmark \- crates.io: Rust Package Registry, 11月 7, 2025にアクセス、 [https://crates.io/crates/pulldown-cmark](https://crates.io/crates/pulldown-cmark)  
5. huggingface/candle: Minimalist ML framework for Rust \- GitHub, 11月 7, 2025にアクセス、 [https://github.com/huggingface/candle](https://github.com/huggingface/candle)  
6. git2 \- crates.io: Rust Package Registry, 11月 7, 2025にアクセス、 [https://crates.io/crates/git2](https://crates.io/crates/git2)  
7. candle-core \- crates.io: Rust Package Registry, 11月 7, 2025にアクセス、 [https://crates.io/crates/candle-core](https://crates.io/crates/candle-core)  
8. lexorank \- crates.io: Rust Package Registry, 11月 7, 2025にアクセス、 [https://crates.io/crates/lexorank/dependencies](https://crates.io/crates/lexorank/dependencies)  
9. pagerank\_rs \- Rust \- Docs.rs, 11月 7, 2025にアクセス、 [https://docs.rs/pagerank\_rs](https://docs.rs/pagerank_rs)  
10. A Simple Text Summarizer written in Rust \- Towards Data Science, 11月 7, 2025にアクセス、 [https://towardsdatascience.com/a-simple-text-summarizer-written-in-rust-4df05f9327a5/](https://towardsdatascience.com/a-simple-text-summarizer-written-in-rust-4df05f9327a5/)  
11. tfidf-text-summarizer \- crates.io: Rust Package Registry, 11月 7, 2025にアクセス、 [https://crates.io/crates/tfidf-text-summarizer](https://crates.io/crates/tfidf-text-summarizer)  
12. libgit2-sys \- crates.io: Rust Package Registry, 11月 7, 2025にアクセス、 [https://crates.io/crates/libgit2-sys](https://crates.io/crates/libgit2-sys)  
13. hppRC/saku: A Japanese Sentence Tokenizer written in Rust. \- GitHub, 11月 7, 2025にアクセス、 [https://github.com/hppRC/saku](https://github.com/hppRC/saku)  
14. lexical-core \- crates.io: Rust Package Registry, 11月 7, 2025にアクセス、 [https://crates.io/crates/lexical-core](https://crates.io/crates/lexical-core)  
15. lexorank \- crates.io: Rust Package Registry, 11月 7, 2025にアクセス、 [https://crates.io/crates/lexorank](https://crates.io/crates/lexorank)  
16. keyword\_extraction \- crates.io: Rust Package Registry, 11月 7, 2025にアクセス、 [https://crates.io/crates/keyword\_extraction](https://crates.io/crates/keyword_extraction)  
17. jieba-rs \- crates.io: Rust Package Registry, 11月 7, 2025にアクセス、 [https://crates.io/crates/jieba-rs](https://crates.io/crates/jieba-rs)  
18. simple-pagerank \- crates.io: Rust Package Registry, 11月 7, 2025にアクセス、 [https://crates.io/crates/simple-pagerank](https://crates.io/crates/simple-pagerank)  
19. Extractive Article Summarization Using Integrated TextRank and BM25+ Algorithm \- MDPI, 11月 7, 2025にアクセス、 [https://www.mdpi.com/2079-9292/12/2/372](https://www.mdpi.com/2079-9292/12/2/372)  
20. Textrank for summarizing text, 11月 7, 2025にアクセス、 [https://cran.r-project.org/web/packages/textrank/vignettes/textrank.html](https://cran.r-project.org/web/packages/textrank/vignettes/textrank.html)