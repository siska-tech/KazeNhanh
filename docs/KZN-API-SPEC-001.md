# **KazeNhanh (カゼニャン) インターフェース設計書 (API設計書)**

**文書ID:** KZN-API-SPEC-001
**バージョン:** 1.0 (草案)  
**ステータス:** 草案 (Draft)
**作成者:** Shion Watanabe
**日付:** 2025/11/7

---

### **0\. API設計の基本原則 (API Design Principles)**

本ライブラリのすべてのインターフェースは、要件定義書（REQ-SPEC-001）の厳格な非機能要件、特にTauri統合とセキュリティ（オフライン保証）を満たすため、以下の設計原則に従わなければならない。

* **原則 0.1: ステートフル・エンジンのカプセル化 (Encapsulation of Stateful Engine)**  
  * **設計:** AIモデル（candle）とNLP辞書（sudachi.rs）は、ロードに時間とメモリを要する「重い」リソースである。これらは、アプリケーションのライフサイクル中に一度だけ初期化されるべきである。  
  * **実装:** すべてのロード済みリソースは、KazeNhanhEngine 構造体にカプセル化される。この構造体は、Tauriの main.rs で初期化され、tauri::Builder::default().manage(engine) を介してTauriの管理対象ステートとして登録されることを唯一かつ必須の利用パターンとする 1。  
  * **保証 (NFR 4.3.1):** KazeNhanhEngine は Send \+ Sync を実装し、Tauriの \#\[tauri::command\] から tauri::State\<'\_, KazeNhanhEngine\> として安全に非同期アクセス可能でなければならない。内部の可変性（例：candleの推論キャッシュ）は Mutex または同等の内部可変性パターンを用いて管理する 2。  
* **原則 0.2: ゼロセットアップのための静的リソース・インジェクション (Static Resource Injection for Zero-Setup)**  
  * **設計 (NFR 4.3.3):** エンドユーザー（Tauriアプリの利用者）は、モデルや辞書のダウンロード、Python環境のセットアップ、その他の外部依存関係の解決を一切要求されてはならない。  
  * **実装:** この要件を満たすため、KazeNhanhEngine のコンストラクタ ::new() は、ファイルパス（\&Path や String）を**一切受け取らない**。代わりに、EngineConfig 構造体を通じて、include\_bytes\! マクロ 3 によってバイナリにコンパイル時に埋め込まれた静的なバイトスライス（&'static \[u8\]）を受け取ることを**強制**する。  
  * **隠蔽される責務:** KazeNhanhEngine::new の内部実装は、これらのバイトスライスを std::io::Cursor （candle のファイルAPIを満たすため 6）や、sudachi.rs の（v0.6.9以降で示唆される 7）組み込みリソースローダーを使って、メモリから直接ロードする責務を負う。これにより、candle 8 や sudachi.rs 9 の（ファイルパスを前提とする）実装詳細をライブラリ利用者に隠蔽する。  
* **原則 0.3: 統一的かつ厳密なエラーハンドリング (Unified & Strict Error Handling)**  
  * **設計 (NFR 4.4.1):** ライブラリの利用者は、Git操作の失敗、モデルのロード失敗、Markdownのパース失敗など、ライブラリ内部で発生しうるすべてのエラーを、単一の明確なエラー型を通じて捕捉できなければならない。  
  * **実装:** ライブラリは thiserror クレート 10 を利用し、pub enum KazeNhanhError を定義する。この列挙型は、\#\[from\] アトリビュート 10 を使用し、基盤となるすべてのエラー（git2::Error, candle\_core::Error, sudachi::Error, std::io::Error）を自動的にラップし、明確なコンテキストを提供する 13。

---

### **1\. 公開API一覧 (Public API List)**

本ライブラリ（クレート: kaze\_nhanh）が公開するすべてのパブリック・インターフェース。

#### **1.1. 名前空間（Namespace） / モジュール（Module）**

* kaze\_nhanh (クレート・ルート)  
* kaze\_nhanh::foundation (基盤的な純粋関数モジュール)

#### **1.2. 主要構造体 (Core Structs)**

* pub struct KazeNhanhEngine:  
  * ライブラリのメイン・インターフェース。Tauriステートとして管理されることを前提とする (NFR 4.3.1)。  
  * 内部にロード済みのcandleモデルとsudachiトークナイザーを保持する。  
* pub struct EngineConfig\<'a\>:  
  * KazeNhanhEngine を構築するための設定。静的なバイトスライス（&'static \[u8\]）のみを受け入れる (原則 0.2)。  
* pub struct GitReportOptions\<'a\>:  
  * generate\_git\_report メソッド（FR 4）の入力パラメータを定義する。  
* pub struct MarkdownSection:  
  * Markdownドキュメントの論理構造（見出し）と行番号範囲のマッピング（FR 2.2）を表現するデータ構造。

#### **1.3. 公開列挙型 (Public Enums)**

* pub enum KazeNhanhError:  
  * ライブラリ全体で使用される統一エラー型 (NFR 4.4.1)。

#### **1.4. KazeNhanhEngine の公開メソッド (Public Methods)**

* pub fn new(config: EngineConfig) \-\> Result\<Self, KazeNhanhError\>:  
  * エンジンのコンストラクタ。高コストな初期化（モデルと辞書のロード）を一度だけ実行する。  
* pub fn generate\_git\_report(\&self, options: GitReportOptions) \-\> Result\<String, KazeNhanhError\>:  
  * **\[キラー機能\]** GitネイティブRAGパイプライン（FR 4）の全体を実行し、自然言語のレポートを生成する。  
* pub fn summarize\_document(\&self, text: \&str) \-\> Result\<String, KazeNhanhError\>:  
  * ハイブリッド要約パイプライン（FR 3）の全体（ステージ1: 抽出 \+ ステージ2: 合成）を実行する。  
* pub fn synthesize\_summary(\&self, sentences: Vec\<String\>) \-\> Result\<String, KazeNhanhError\>:  
  * ハイブリッド要約のステージ2（合成）のみを独立して実行する (FR 3.3.2のモジュール性要件)。  
* pub fn extract\_important\_sentences(\&self, text: \&str, count: usize) \-\> Result\<Vec\<String\>, KazeNhanhError\>:  
  * ハイブリッド要約のステージ1（抽出）のみを独立して実行する (FR 3.3.1)。  
  * （*アーキテクト注: これは sudachi に依存するため、KazeNhanhEngine のメソッドでなければならない*）  
* pub fn tokenize\_sentence(\&self, sentence: \&str) \-\> Result\<Vec\<Morpheme\>, KazeNhanhError\>:  
  * 基盤機能（FR 1.2）へのアクセス。sudachi.rs を利用して形態素解析を実行する。  
  * （*Morpheme は sudachi::Morpheme のラッパー、または sudachi::Morpheme の再エクスポートとする*）

#### **1.5. 公開関数（Foundation Modules）**

* pub mod foundation::nlp:  
  * pub fn split\_sentences(text: \&str) \-\> Vec\<String\>:  
    * 基盤機能（FR 1.1）。saku クレート 15 を利用した文分割。状態（Engine）に依存しないため、純粋関数として提供する。  
* pub mod foundation::markdown:  
  * pub fn map\_document\_structure(markdown\_text: \&str) \-\> Result\<Vec\<MarkdownSection\>, KazeNhanhError\>:  
    * 基盤機能（FR 2）。pulldown-cmark (REQ 5.4) を利用した構造と行番号のマッピング。状態に依存しない純粋関数。

---

### **2\. 各APIの詳細定義 (Detailed API Definitions)**

#### **2.1. コア・エンジンと設定 (Core Engine & Configuration)**

##### **pub struct EngineConfig\<'a\>**

* **概要:** KazeNhanhEngine を初期化するために必要なすべてのリソースを、静的なバイトスライスとして定義する。ファイルパスは意図的に排除されている（原則 0.2）。  
* **シグネチャ (フィールド):**  
  Rust  
  pub struct EngineConfig\<'a\> {  
      /// candleでロードするGGUFモデルのバイナリデータ  
      /// (REQ 5.6: GGUF)  
      pub model\_bytes: &'a \[u8\],

      /// sudachi.rs が使用するシステム辞書のバイナリデータ (system.dic)  
      /// (REQ 5.5: sudachi.rs)  
      pub dictionary\_bytes: &'a \[u8\],

      /// sudachi.rs が使用する設定ファイル (sudachi.json) のバイナリデータ  
      /// (REQ 5.5: sudachi.rs)  
      pub settings\_bytes: &'a \[u8\],

      // 将来的な拡張（例：ユーザー辞書）のために予約  
      // pub user\_dictionary\_bytes: Option\<&'a \[u8\]\>,  
  }

  impl\<'a\> EngineConfig\<'a\> {  
      /// 必須リソースから新しい設定を作成するヘルパー関数  
      pub fn new(  
          model\_bytes: &'a \[u8\],  
          dictionary\_bytes: &'a \[u8\],  
          settings\_bytes: &'a \[u8\]  
      ) \-\> Self {  
          Self {  
              model\_bytes,  
              dictionary\_bytes,  
              settings\_bytes,  
          }  
      }  
  }

* **使用例:** (セクション 3.1 を参照)

##### **pub struct KazeNhanhEngine**

* **概要:** ロード済みのAIモデルとNLP辞書を保持する、Tauriステート管理用のメイン構造体（原則 0.1）。この構造体のインスタンスが、すべてのコア機能のレシーバ（\&self）となる。  
* **シグネチャ:**  
  Rust  
  // 内部フィールドは公開されない (private)  
  pub struct KazeNhanhEngine {  
      // AIモデル推論器 (candle)  
      // Mutexは、Tauriの非同期コマンド間でのスレッドセーフなアクセスを保証する  
      internal\_model: std::sync::Arc\<std::sync::Mutex\<Box\<dyn KazeNhanhModel\>\>\>,

      // 日本語トークナイザー (sudachi)  
      // Tokenizerは内部的にArcを使用していることが多いため、Mutexは不要かもしれないが、  
      // 安全のためArcでラップする  
      internal\_tokenizer: std::sync::Arc\<sudachi::Tokenizer\>,

      // その他のキャッシュや設定  
  }

  // KazeNhanhModelは、candleのモデル（Llama, Phi-3等）を抽象化する  
  // ライブラリ内部の (pub(crate)) トレイト  
  trait KazeNhanhModel: Send \+ Sync {  
      fn synthesize(&mut self, sentences: Vec\<String\>) \-\> Result\<String, candle\_core::Error\>;  
  }

##### **pub fn KazeNhanhEngine::new(config: EngineConfig) \-\> Result\<Self, KazeNhanhError\>**

* **シグネチャ:** pub fn new(config: EngineConfig) \-\> Result\<Self, KazeNhanhError\>  
* **概要:** EngineConfig で提供されたバイトスライスから、すべてのリソース（AIモデル、辞書）をメモリ上にロードし、推論可能な KazeNhanhEngine インスタンスを構築する。これは高コストな操作であり、アプリケーション起動時に一度だけ呼び出すこと（原則 0.1）。  
* **引数の詳細:**  
  * config: EngineConfig: include\_bytes\! マクロでロードされたAIモデルとSudachi辞書のバイナリデータを含む設定。  
* **戻り値の詳細:**  
  * Ok(KazeNhanhEngine): 正常に初期化されたエンジン。  
  * Err(KazeNhanhError): リソースのロードまたはパースに失敗した場合。  
* **例外・エラーハンドリング:**  
  * KazeNhanhError::ModelLoadError: config.model\_bytes (GGUF) のパースまたはロードに失敗した場合。  
    * *実装上の洞察:* candle が &\[u8\] から直接ロードできない場合、std::io::Cursor::new(config.model\_bytes) を作成し、gguf\_file::Content::read のようなファイルAPIを模倣してロードする必要がある 6。candle\_transformers::quantized\_var\_builder 16 を活用する。  
  * KazeNhanhError::DictionaryLoadError: config.dictionary\_bytes または config.settings\_bytes (Sudachi) のパースまたはロードに失敗した場合。  
    * *実装上の洞察:* sudachi.rs v0.6.9で追加された「組み込みコンフィグとフォールバックリソース」機能 7 を利用し、メモリから辞書をロードする。この機能の存在が、NFR 4.3.3（ゼロセットアップ）の達成に不可欠である。  
  * KazeNhanhError::IoError: Cursor の操作など、予期せぬIOエラーが発生した場合。

---

#### **2.2. エラーハンドリング (Error Handling)**

##### **pub enum KazeNhanhError**

* **概要:** ライブラリ全体の操作で発生しうるすべてのエラーを網羅する、thiserror 11 ベースの統一エラー列挙型 (NFR 4.4.1)。  
* **シグネチャ (定義):**  
  Rust  
  use thiserror::Error;

  \#  
  pub enum KazeNhanhError {  
      \#\[error("Gitリポジトリの操作に失敗しました (パス: {path})")\]  
      GitOperationError {  
          path: String,  
          \#\[source\]  
          source: git2::Error,  
      },

      \#\[error("AIモデルのロードに失敗しました")\]  
      ModelLoadError {  
          \#\[source\]  
          source: candle\_core::Error,  
      },

      \#\[error("AIモデルでの推論（合成）中にエラーが発生しました")\]  
      ModelInferenceError {  
          \#\[source\]  
          source: candle\_core::Error,  
      },

      \#\[error("日本語辞書または設定ファイルのロードに失敗しました")\]  
      DictionaryLoadError {  
          \#\[source\]  
          source: sudachi::Error,  
      },

      \#\[error("日本語の形態素解析に失敗しました")\]  
      TokenizationError {  
          \#\[source\]  
          source: sudachi::Error,  
      },

      \#\[error("Markdownの解析または行番号のマッピングに失敗しました: {0}")\]  
      MarkdownParseError(String),

      \#\[error("Git差分(diff)の解析に失敗しました: {0}")\]  
      DiffParseError(String),

      \#  
      SummarizeEngineError(String),

      \#\[error("リポジトリが見つかりません: {0}")\]  
      RepositoryNotFound(String),

      \#\[error("無効な入力が指定されました: {0}")\]  
      InvalidInput(String),

      \#\[error("予期せぬI/Oエラーが発生しました")\]  
      IoError(\#\[from\] std::io::Error),  
  }

* **表1: KazeNhanhError のバリアントと発生源**  
  * **設計的価値:** このテーブルは、ライブラリ利用者が match 式でエラーを処理する際に、どのエラーがどのAPIから、どのような基盤ライブラリ（git2, candleなど）に起因して発生するかを明確に対応付けるために不可欠である。

| バリアント | 関連する機能要件 (FR) | 発生源 (API) | 基盤となるエラー (\#\[from\] / source) |
| :---- | :---- | :---- | :---- |
| GitOperationError | FR 4.1, 4.2 | generate\_git\_report | git2::Error (REQ 5.3) |
| ModelLoadError | (初期化) | KazeNhanhEngine::new | candle\_core::Error (REQ 5.2) |
| ModelInferenceError | FR 3.3.2, FR 4.5 | synthesize\_summary | candle\_core::Error (REQ 5.2) |
| DictionaryLoadError | (初期化) | KazeNhanhEngine::new | sudachi::Error (REQ 5.5) |
| TokenizationError | FR 1.2, FR 3.3.1 | tokenize\_sentence, extract\_important\_sentences | sudachi::Error (REQ 5.5) |
| MarkdownParseError | FR 2.2, FR 4.3 | map\_document\_structure, generate\_git\_report | (内部ロジック, pulldown-cmark (REQ 5.4)) |
| DiffParseError | FR 4.2 | generate\_git\_report | (内部ロジック, git2::Error (REQ 5.3)) |
| SummarizeEngineError | FR 3.3.1 | extract\_important\_sentences | (内部ロジック, petgraph 17 / scirs2-text 18) |
| RepositoryNotFound | FR 4.1 | generate\_git\_report | git2::Error (REQ 5.3) |
| InvalidInput | (全般) | (全般) | (内部バリデーション) |
| IoError | (初期化, 全般) | KazeNhanhEngine::new | std::io::Error |

---

#### **2.3. FR 4: GitネイティブRAG パイプライン (Git-Native RAG)**

##### **pub struct GitReportOptions\<'a\>**

* **概要:** GitネイティブRAGパイプライン（FR 4）の実行に必要なパラメータを定義する。  
* **シグネチャ (フィールド):**  
  Rust  
  pub struct GitReportOptions\<'a\> {  
      /// 解析対象のローカルGitリポジトリのルートパス  
      /// (例: "C:\\\\Users\\\\dev\\\\Documents\\\\MyNotes")  
      pub repo\_path: &'a str,

      /// 何日前からの差分を対象とするか (例: 7\)  
      /// (REQ 3.4.1)  
      pub days\_since: u32,

      /// 解析対象とするMarkdownファイルの拡張子 (例: ".md")  
      /// デフォルトは ".md" と ".markdown"  
      pub target\_extensions: Option\<Vec\<&'a str\>\>,

      /// AIに渡すカスタムプロンプト（オプション）  
      /// Noneの場合、REQ 3.4.5の形式に準拠したデフォルトプロンプトが使用される  
      pub custom\_prompt: Option\<&'a str\>,  
  }

##### **impl KazeNhanhEngine { pub fn generate_git_report(&self, options: GitReportOptions) -> Result<String, KazeNhanhError> }**

* **シグネチャ:** pub fn generate_git_report(&self, options: GitReportOptions) -> Result<String, KazeNhanhError>
* **概要:** FR 4で定義されたGitネイティブRAGの5ステップ・パイプライン（差分検出、変更箇所特定、文脈特定、相関、レポート生成）をすべて実行し、AIによって生成された自然言語のサマリー（REQ 3.4.5の形式）を返す。
* **実装メモ (2025-11-07):** 現段階では `foundation::git_service::collect_markdown_diffs` の結果をMarkdown整形して返す簡易レポートを提供する。AIサマリー統合は `task-pipeline-002` で実装予定。
* **引数の詳細:**

##### **impl KazeNhanhEngine { pub fn summarize_document(&self, text: &str) -> Result<String, KazeNhanhError> }**

* **シグネチャ:** pub fn summarize_document(&self, text: &str) -> Result<String, KazeNhanhError>
* **概要:** ハイブリッド要約（FR 3）の「Drip and Stir」パイプラインをエンド・ツー・エンドで実行する。ステージ1（extract_important_sentences）とステージ2（synthesize_summary）を内部で連続して呼び出す。
* **実装メモ (2025-11-07):** Sudachiトークナイズ結果を軽量プロンプトに反映し、推論エンジンの暫定 `synthesize` に委譲する簡易要約実装。LexRank + LLM 構成は `task-pipeline-001` で導入予定。
* **引数の詳細:**

##### **impl KazeNhanhEngine { pub fn synthesize_summary(&self, sentences: Vec<String>) -> Result<String, KazeNhanhError> }**

* **シグネチャ:** pub fn synthesize_summary(&self, sentences: Vec<String>) -> Result<String, KazeNhanhError>
* **概要:** ステージ2「合成 (Stir)」（FR 3.3.2）のみを独立して実行する。抽出された（可能性のある）バラバラの重要文のリストを受け取り、AI（SLM）で自然な段落に「肉付け」し、再構成する。FR 4.5（レポート生成）からも内部的に利用される。
* **実装メモ (2025-11-07):** 受け取った文を改行で連結し、推論エンジンの `synthesize` にそのまま渡す暫定構成。温度制御やログits処理は後続タスクで追加予定。
* **引数の詳細:**

##### **impl KazeNhanhEngine { pub fn extract_important_sentences(&self, text: &str, count: usize) -> Result<Vec<String>, KazeNhanhError> }**

* **シグネチャ:** pub fn extract_important_sentences(&self, text: &str, count: usize) -> Result<Vec<String>, KazeNhanhError>
* **概要:** ステージ1「抽出 (Drip)」（FR 3.3.1）。AIを使用せず、LexRank/TextRankアルゴリズムに基づき、ドキュメントから重要文を高速に抽出する。
* **実装メモ (2025-11-07):** 句読点ベースの分割による簡易抽出を一時的に使用。LexRank 実装は `task-pipeline-001` の成果物で置き換える。
* **実装上の洞察:** この関数は内部で以下の処理を実行する 23。

---

#### **2.4. FR 3: ハイブリッド要約エンジン (Hybrid Summarization Engine)**

##### **impl KazeNhanhEngine { pub fn summarize\_document(\&self, text: \&str) \-\> Result\<String, KazeNhanhError\> }**

* **シグネチャ:** pub fn summarize\_document(\&self, text: \&str) \-\> Result\<String, KazeNhanhError\>  
* **概要:** ハイブリッド要約（FR 3）の「Drip and Stir」パイプラインをエンド・ツー・エンドで実行する。ステージ1（extract\_important\_sentences）とステージ2（synthesize\_summary）を内部で連続して呼び出す。  
* **引数の詳細:**  
  * \&self: KazeNhanhEngine インスタンス。ステージ1（Sudachi）とステージ2（Candle）の両方を使用する。  
  * text: \&str: 要約対象の日本語の全文テキスト。  
* **戻り値の詳細:**  
  * Ok(String): AIによって合成された、自然な日本語の要約パラグラフ。  
  * Err(KazeNhanhError): 抽出または合成のいずれかで失敗した場合。  
* **例外・エラーハンドリング:**  
  * KazeNhanhError::SummarizeEngineError: ステージ1（抽出）のLexRank計算に失敗した場合。  
  * KazeNhanhError::ModelInferenceError: ステージ2（合成）のAI推論に失敗した場合。  
  * KazeNhanhError::TokenizationError: ステージ1の前処理（sudachi）に失敗した場合。

##### **impl KazeNhanhEngine { pub fn synthesize\_summary(\&self, sentences: Vec\<String\>) \-\> Result\<String, KazeNhanhError\> }**

* **シグネチャ:** pub fn synthesize\_summary(\&self, sentences: Vec\<String\>) \-\> Result\<String, KazeNhanhError\>  
* **概要:** ステージ2「合成 (Stir)」（FR 3.3.2）のみを独立して実行する。抽出された（可能性のある）バラバラの重要文のリストを受け取り、AI（SLM）で自然な段落に「肉付け」し、再構成する。FR 4.5（レポート生成）からも内部的に利用される。  
* **引数の詳細:**  
  * \&self: KazeNhanhEngine インスタンス。candle モデルを使用する。  
  * sentences: Vec\<String\>: AIに入力として渡す、抽出済みの文のリスト。  
* **戻り値の詳細:**  
  * Ok(String): AIによって合成された、自然な日本語の要約パラグラフ。  
  * Err(KazeNhanhError): AI推論が失敗した場合。  
* **例外・エラーハンドリング:**  
  * KazeNhanhError::ModelInferenceError: candle での推論実行（forward 呼び出し）に失敗した場合。

---

#### **2.5. FR 1 & 2 & 3.1: 基盤コンポーネント (Foundation Components)**

##### **pub mod foundation::nlp { pub fn split\_sentences(text: \&str) \-\> Vec\<String\> }**

* **シグネチャ:** pub fn split\_sentences(text: \&str) \-\> Vec\<String\>  
* **概要:** 日本語NLP基盤（FR 1.1）。入力テキストを文法的に正確な「文」のリストに分割する。saku Crate 15 のラッパーであり、状態を持たない純粋関数。  
* **引数の詳細:**  
  * text: \&str: 分割対象の日本語テキスト。  
* **戻り値の詳細:**  
  * Vec\<String\>: 分割された文のリスト。  
* **例外・エラーハンドリング:** (なし。saku がパニックしない限り、常に Vec を返す)

##### **impl KazeNhanhEngine { pub fn tokenize\_sentence(\&self, sentence: \&str) \-\> Result\<Vec\<Morpheme\>, KazeNhanhError\> }**

* **シグネチャ:** pub fn tokenize\_sentence(\&self, sentence: \&str) \-\> Result\<Vec\<Morpheme\>, KazeNhanhError\>  
  * （*Morpheme は sudachi::Morpheme のエイリアス、または sudachi クレートを再エクスポートする*）  
* **概要:** 日本語NLP基盤（FR 1.2）。入力された「文」を形態素（トークン）に分割し、品詞情報を特定する。ロード済みの sudachi 辞書（internal\_tokenizer）に依存するため、KazeNhanhEngine のメソッドとして提供する。  
* **引数の詳細:**  
  * \&self: KazeNhanhEngine インスタンス。sudachi トークナイザーを使用する。  
  * sentence: \&str: 形態素解析対象の単一の文。  
* **戻り値の詳細:**  
  * Ok(Vec\<Morpheme\>): 形態素のリスト。  
  * Err(KazeNhanhError::TokenizationError): sudachi での解析に失敗した場合。

##### **impl KazeNhanhEngine { pub fn extract\_important\_sentences(\&self, text: \&str, count: usize) \-\> Result\<Vec\<String\>, KazeNhanhError\> }**

* **シグネチャ:** pub fn extract\_important\_sentences(\&self, text: \&str, count: usize) \-\> Result\<Vec\<String\>, KazeNhanhError\>  
* **概要:** ステージ1「抽出 (Drip)」（FR 3.3.1）。AIを使用せず、LexRank/TextRankアルゴリズムに基づき、ドキュメントから重要文を高速に抽出する。  
* **実装上の洞察:** この関数は内部で以下の処理を実行する 23。  
  1. foundation::nlp::split\_sentences で文に分割。  
  2. 各文を self.tokenize\_sentence で形態素解析（sudachi）。  
  3. 形態素に基づき、各文をTF-IDFベクトル化（例：scirs2-text 18 の利用）。  
  4. 文同士のコサイン類似度マトリックス（N x N）を構築。  
  5. 類似度マトリックスをグラフとみなし、PageRankアルゴリズム（例：petgraph::algo::page\_rank 17）を実行して各文のセントラリティ（重要度）を計算。  
  6. スコアに基づき、上位 count 件の文を返す。  
* **引数の詳細:**  
  * \&self: KazeNhanhEngine インスタンス。sudachi トークナイザーを使用する。  
  * text: \&str: 抽出対象の日本語の全文テキスト。  
  * count: usize: 抽出する文の最大数（例：5）。  
* **戻り値の詳細:**  
  * Ok(Vec\<String\>): 抽出された重要文のリスト（順序は重要度順、または元の出現順）。  
  * Err(KazeNhanhError::SummarizeEngineError): TF-IDFまたはPageRankの計算に失敗した場合。  
  * Err(KazeNhanhError::TokenizationError): sudachi での解析に失敗した場合。

##### **pub struct foundation::markdown::MarkdownSection**

* **概要:** Markdownドキュメントの論理構造（セクション）と、それがドキュメントの物理的な行番号のどこに対応するかをマッピングするデータ構造（FR 2.2）。  
* **シグネチャ (フィールド):**  
  Rust  
  \#  
  pub struct MarkdownSection {  
      /// 見出しのテキスト（例: "完了したタスク"）  
      pub heading\_text: String,

      /// 見出しのレベル (H1=1, H2=2,...)  
      pub level: u32,

      /// このセクションが開始する行番号 (1-based)  
      pub start\_line: usize,

      /// このセクションが終了する行番号 (1-based)  
      /// (次の見出しの開始行 \- 1、またはドキュメントの最終行)  
      pub end\_line: usize,  
  }

##### **pub mod foundation::markdown { pub fn map\_document\_structure(markdown\_text: \&str) \-\> Result\<Vec\<MarkdownSection\>, KazeNhanhError\> }**

* **シグネチャ:** pub fn map\_document\_structure(markdown\_text: \&str) \-\> Result\<Vec\<MarkdownSection\>, KazeNhanhError\>  
* **概要:** Markdown解析基盤（FR 2）。pulldown-cmark (REQ 5.4) を利用し、Markdownテキストを解析して Vec\<MarkdownSection\> を返す。FR 4.3（文脈の特定）に不可欠。  
* **実装上の洞察:** FR 2.2の「行番号」要件を満たすため、本関数は pulldown\_cmark::Parser の into\_offset\_iter() 25 を使用する。  
  1. into\_offset\_iter() はバイトオフセット（Range\<usize\>）を返す 26。  
  2. このバイトオフセットを「行番号」に変換するため、最初に入力 markdown\_text を行ごとにイテレートし、各行の開始バイトオフセットを Vec\<usize\> にキャッシュする。  
  3. pulldown-cmark の Event::Start(Tag::Heading(..)) 27 が検出された際、そのイベントのバイトオフセット（Range.start）をキャッシュした Vec で二分探索（binary\_search）し、対応する start\_line を特定する。  
* **引数の詳細:**  
  * markdown\_text: \&str: 解析対象のMarkdownドキュメント全文。  
* **戻り値の詳細:**  
  * Ok(Vec\<MarkdownSection\>): ドキュメント内の全セクションの構造と行番号マッピングのリスト。  
  * Err(KazeNhanhError::MarkdownParseError): オフセットと行番号のマッピングに失敗した場合（内部ロジックエラー）。

---

### **3\. 使用例 (Code Snippets)**

#### **3.1. \[最重要\] Tauri との統合とEngineの初期化 (Tauri Integration and Engine Initialization)**

REQ 4.3 (Tauriファースト)、NFR 4.3.3 (ゼロセットアップ)、および原則 0.2 (静的リソース) を満たすための、Tauriアプリケーション (main.rs) における標準的な初期化シーケンス。

Rust

// main.rs (Tauri バックエンド)

// KazeNhanhライブラリのコアコンポーネントをインポート  
use kaze\_nhanh::{KazeNhanhEngine, EngineConfig, KazeNhanhError, GitReportOptions};

// 1\. (REQ 5.6, NFR 4.3.3)  
// \`include\_bytes\!\` を使用し、AIモデルと辞書をバイナリに静的に埋め込む \[3, 4\]  
// これらのファイルは、開発者がTauriの \`src-tauri\` フォルダ等に配置する必要がある  
const MODEL\_BYTES: &'static \[u8\] \=   
    include\_bytes\!("../models/llama-3-elyza-jp-8b-q4\_k\_m.gguf"); // REQ 5.6  
const SUDACHI\_DICT\_BYTES: &'static \[u8\] \=   
    include\_bytes\!("../models/sudachi-dictionary-core-20240101.dic"); // REQ 5.5  
const SUDACHI\_SETTINGS\_BYTES: &'static \[u8\] \=   
    include\_bytes\!("../models/sudachi.json"); // REQ 5.5

// KazeNhanhEngineのインスタンスを保持するTauriステート  
// (KazeNhanhEngine自体が Send \+ Sync を実装しているため、直接 manage しても良い)  
struct AppState {  
    engine: KazeNhanhEngine,  
}

fn main() {  
    // 2\. (原則 0.2)  
    // ファイルパスではなく、静的なバイトスライスを使用してEngineConfigを構築  
    let config \= EngineConfig::new(  
        MODEL\_BYTES,  
        SUDACHI\_DICT\_BYTES,  
        SUDACHI\_SETTINGS\_BYTES  
    );

    // 3\. (原則 0.1)  
    // アプリケーション起動時に一度だけ、高コストなエンジン初期化を実行  
    let engine \= KazeNhanhEngine::new(config)  
       .expect("KazeNhanh Engineのロードに失敗しました。モデルまたは辞書ファイルを確認してください。");

    let app\_state \= AppState { engine };

    // 4\. (REQ 4.3.1, NFR 4.3.1)  
    // エンジン・インスタンスをTauriの管理対象ステートに追加する   
    tauri::Builder::default()  
       .manage(app\_state) // Tauriが \`AppState\` をグローバルに管理  
       .invoke\_handler(tauri::generate\_handler\!\[  
            generate\_git\_report\_command,  
            summarize\_document\_command  
        \])  
       .run(tauri::generate\_context\!())  
       .expect("Tauriアプリケーションの実行に失敗しました。");  
}

// Tauriコマンドの定義 (セクション 3.2, 3.3 へ続く)

#### **3.2. Tauri コマンド: Git差分レポートの生成 (Tauri Command: Generating a Git Report)**

FR 4（GitネイティブRAG）をTauriフロントエンドから呼び出すためのコマンド例。

Rust

// main.rs (続き)

// Tauriステートから KazeNhanhEngine を安全に受け取る   
\#\[tauri::command\]  
async fn generate\_git\_report\_command(  
    repo\_path: String,  
    days\_since: u32,  
    state: tauri::State\<'\_, AppState\>  
) \-\> Result\<String, String\> {  
      
    // 1\. フロントエンドからの入力を KazeNhanh のAPIにマッピング  
    let options \= GitReportOptions {  
        repo\_path: \&repo\_path,  
        days\_since: days\_since,  
        target\_extensions: None, // デフォルトを使用  
        custom\_prompt: None,  
    };

    // 2\. (FR 4\)  
    // メインエンジンのキラー機能を呼び出す  
    // \`state.engine\` は \`KazeNhanhEngine\` のインスタンス  
    let report \= state.engine.generate\_git\_report(options)  
       .map\_err(|e| e.to\_string())?; // エラーをフロントエンドに送信可能なStringに変換

    Ok(report)  
}

#### **3.3. Tauri コマンド: ドキュメントのハイブリッド要約 (Tauri Command: Hybrid Summarization)**

FR 3（ハイブリッド要約）をTauriフロントエンドから呼び出すためのコマンド例。

Rust

// main.rs (続き)

\#\[tauri::command\]  
async fn summarize\_document\_command(  
    full\_text: String,  
    state: tauri::State\<'\_, AppState\>  
) \-\> Result\<String, String\> {  
      
    // (FR 3\)  
    // エンジンをTauriステートから取得し、要約を実行  
    let summary \= state.engine.summarize\_document(\&full\_text)  
       .map\_err(|e| e.to\_string())?;

    Ok(summary)  
}

#### **3.4. AI合成機能の個別利用 (Standalone Use of AI Synthesizer)**

FR 3.3.2のモジュール性要件を満たすため、利用者が独自の「抽出（Drip）」ロジックを使い、KazeNhanhの「合成（Stir）」機能のみを利用する例。

Rust

// 独自の抽出ロジック（例：正規表現で "【重要】" タグを探す）  
fn my\_custom\_extractor(text: &str) \-\> Vec\<String\> {  
    text.lines()  
       .filter(|line| line.starts\_with("【重要】"))  
       .map(|line| line.trim\_start\_matches("【重要】").to\_string())  
       .collect()  
}

// TauriコマンドまたはTauriバックエンド内の別ロジック  
fn run\_custom\_summary(  
    text\_to\_analyze: &str,  
    engine: \&KazeNhanhEngine // (TauriのStateなどから取得)  
) \-\> Result\<String, kaze\_nhanh::KazeNhanhError\> {  
      
    // 1\. 独自のロジックで文を抽出  
    let important\_sentences \= my\_custom\_extractor(text\_to\_analyze);

    if important\_sentences.is\_empty() {  
        return Ok("要約対象の重要項目が見つかりませんでした。".to\_string());  
    }

    // 2\. (FR 3.3.2)  
    // KazeNhanh のAI合成機能のみを呼び出し、"肉付け" を依頼  
    engine.synthesize\_summary(important\_sentences)  
}

#### **3.5. 基盤APIの利用 (Using Foundation APIs)**

KazeNhanhEngine の初期化を必要としない、または Engine を使って低レベルAPIを呼び出す例。

Rust

use kaze\_nhanh::foundation::{nlp, markdown};  
use kaze\_nhanh::{KazeNhanhEngine, KazeNhanhError};

fn analyze\_foundations(  
    engine: \&KazeNhanhEngine, // (TauriのStateなどから取得)  
    markdown\_doc: &str,  
    japanese\_text: &str  
) \-\> Result\<(), KazeNhanhError\> {

    // 1\. (FR 1.1) 文分割 (純粋関数)  
    // \`saku\`  のラッパー  
    let sentences \= nlp::split\_sentences(japanese\_text);  
    println\!("文の数: {}", sentences.len());

    // 2\. (FR 1.2) 形態素解析 (Engineメソッド)  
    // \`sudachi\` (REQ 5.5) に依存  
    if let Some(first\_sentence) \= sentences.first() {  
        let morphemes \= engine.tokenize\_sentence(first\_sentence)?;  
        println\!("最初の文の形態素: {:?}", morphemes.len());  
    }

    // 3\. (FR 2\) Markdown構造解析 (純粋関数)  
    // \`pulldown-cmark\`  のラッパー  
    let structure \= markdown::map\_document\_structure(markdown\_doc)?;  
      
    if let Some(section) \= structure.first() {  
        // (FR 2.2)  
        println\!(  
            "最初のセクション: '{}' (L{}, {}行目-{}行目)",  
            section.heading\_text,  
            section.level,  
            section.start\_line,  
            section.end\_line  
        );  
    }

    Ok(())  
}

#### **引用文献**

1. Manager in tauri \- Rust \- Docs.rs, 11月 7, 2025にアクセス、 [https://docs.rs/tauri/latest/tauri/trait.Manager.html](https://docs.rs/tauri/latest/tauri/trait.Manager.html)  
2. State Management \- Tauri, 11月 7, 2025にアクセス、 [https://v2.tauri.app/develop/state-management/](https://v2.tauri.app/develop/state-management/)  
3. Generate concatinated path for include\_bytes at compile time \- help \- Rust Users Forum, 11月 7, 2025にアクセス、 [https://users.rust-lang.org/t/generate-concatinated-path-for-include-bytes-at-compile-time/73930](https://users.rust-lang.org/t/generate-concatinated-path-for-include-bytes-at-compile-time/73930)  
4. How to embed resources in Rust executable? \- Stack Overflow, 11月 7, 2025にアクセス、 [https://stackoverflow.com/questions/27140634/how-to-embed-resources-in-rust-executable](https://stackoverflow.com/questions/27140634/how-to-embed-resources-in-rust-executable)  
5. Can i conveniently compile bytes into a Rust program with a specific alignment?, 11月 7, 2025にアクセス、 [https://users.rust-lang.org/t/can-i-conveniently-compile-bytes-into-a-rust-program-with-a-specific-alignment/24049](https://users.rust-lang.org/t/can-i-conveniently-compile-bytes-into-a-rust-program-with-a-specific-alignment/24049)  
6. Candle | Rust for DS and DE, 11月 7, 2025にアクセス、 [https://rust.marcoinacio.com/data/candle/](https://rust.marcoinacio.com/data/candle/)  
7. Releases · WorksApplications/sudachi.rs \- GitHub, 11月 7, 2025にアクセス、 [https://github.com/WorksApplications/sudachi.rs/releases](https://github.com/WorksApplications/sudachi.rs/releases)  
8. Example quantized with custom GGUF model error: cannot find llama.attention.head\_count in metadata · Issue \#2450 · huggingface/candle \- GitHub, 11月 7, 2025にアクセス、 [https://github.com/huggingface/candle/issues/2450](https://github.com/huggingface/candle/issues/2450)  
9. WorksApplications/sudachi.rs: Sudachi in Rust and new generation of SudachiPy \- GitHub, 11月 7, 2025にアクセス、 [https://github.com/WorksApplications/sudachi.rs](https://github.com/WorksApplications/sudachi.rs)  
10. Correct way to use "?;" syntax with multiple error types? \- Stack Overflow, 11月 7, 2025にアクセス、 [https://stackoverflow.com/questions/76694348/correct-way-to-use-syntax-with-multiple-error-types](https://stackoverflow.com/questions/76694348/correct-way-to-use-syntax-with-multiple-error-types)  
11. A Comprehensive Guide to robust code with thiserror for Rust | by loudsilence | Rustaceans | Medium, 11月 7, 2025にアクセス、 [https://medium.com/rustaceans/a-comprehensive-guide-to-robust-code-with-thiserror-for-rust-43778b1b3906](https://medium.com/rustaceans/a-comprehensive-guide-to-robust-code-with-thiserror-for-rust-43778b1b3906)  
12. thiserror \- Rust \- Docs.rs, 11月 7, 2025にアクセス、 [https://docs.rs/thiserror](https://docs.rs/thiserror)  
13. Idiomatically handle multiple error types \- rust \- Reddit, 11月 7, 2025にアクセス、 [https://www.reddit.com/r/rust/comments/ikmufi/idiomatically\_handle\_multiple\_error\_types/](https://www.reddit.com/r/rust/comments/ikmufi/idiomatically_handle_multiple_error_types/)  
14. Understanding Best Practices for propagating different errors in libraries \- Rust Users Forum, 11月 7, 2025にアクセス、 [https://users.rust-lang.org/t/understanding-best-practices-for-propagating-different-errors-in-libraries/120269](https://users.rust-lang.org/t/understanding-best-practices-for-propagating-different-errors-in-libraries/120269)  
15. hppRC/saku: A Japanese Sentence Tokenizer written in Rust. \- GitHub, 11月 7, 2025にアクセス、 [https://github.com/hppRC/saku](https://github.com/hppRC/saku)  
16. candle\_transformers \- Rust \- Docs.rs, 11月 7, 2025にアクセス、 [https://docs.rs/candle-transformers/](https://docs.rs/candle-transformers/)  
17. petgraph::algo::page\_rank \- Rust \- Docs.rs, 11月 7, 2025にアクセス、 [https://docs.rs/petgraph/latest/petgraph/algo/page\_rank/index.html](https://docs.rs/petgraph/latest/petgraph/algo/page_rank/index.html)  
18. scirs2-text \- crates.io: Rust Package Registry, 11月 7, 2025にアクセス、 [https://crates.io/crates/scirs2-text](https://crates.io/crates/scirs2-text)  
19. Diff in git2 \- Rust \- Docs.rs, 11月 7, 2025にアクセス、 [https://docs.rs/git2/latest/git2/struct.Diff.html](https://docs.rs/git2/latest/git2/struct.Diff.html)  
20. git\_diff\_hunk (libgit2 main), 11月 7, 2025にアクセス、 [https://libgit2.org/docs/reference/main/diff/git\_diff\_hunk.html](https://libgit2.org/docs/reference/main/diff/git_diff_hunk.html)  
21. diff.rs \- source \- Docs.rs, 11月 7, 2025にアクセス、 [https://docs.rs/git2/latest/src/git2/diff.rs.html](https://docs.rs/git2/latest/src/git2/diff.rs.html)  
22. saku \- crates.io: Rust Package Registry, 11月 7, 2025にアクセス、 [https://crates.io/crates/saku](https://crates.io/crates/saku)  
23. lexRankr: Extractive Text Summariztion in R \- README, 11月 7, 2025にアクセス、 [https://cran.r-project.org/web/packages/lexRankr/readme/README.html](https://cran.r-project.org/web/packages/lexRankr/readme/README.html)  
24. A Simple Text Summarizer written in Rust \- Towards Data Science, 11月 7, 2025にアクセス、 [https://towardsdatascience.com/a-simple-text-summarizer-written-in-rust-4df05f9327a5/](https://towardsdatascience.com/a-simple-text-summarizer-written-in-rust-4df05f9327a5/)  
25. pulldown-cmark guide, 11月 7, 2025にアクセス、 [https://pulldown-cmark.github.io/pulldown-cmark/](https://pulldown-cmark.github.io/pulldown-cmark/)  
26. Pulldown-cmark (CommonMark in Rust) \- Implementation, 11月 7, 2025にアクセス、 [https://talk.commonmark.org/t/pulldown-cmark-commonmark-in-rust/1205](https://talk.commonmark.org/t/pulldown-cmark-commonmark-in-rust/1205)  
27. Tag in pulldown\_cmark \- Rust \- Docs.rs, 11月 7, 2025にアクセス、 [https://docs.rs/pulldown-cmark/latest/pulldown\_cmark/enum.Tag.html](https://docs.rs/pulldown-cmark/latest/pulldown_cmark/enum.Tag.html)