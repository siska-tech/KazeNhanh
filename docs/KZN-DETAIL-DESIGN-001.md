# **詳細設計書 (モジュール設計書)**

**文書ID:** KZN-DETAIL-DESIGN-001
**バージョン:** 1.0 (草案)  
**ステータス:** 草案 (Draft)
**作成者:** Shion Watanabe
**日付:** 2025/11/7

## **序文: 本書の目的とスコープ**

本設計書（DDD）は、KazeNhanhライブラリの技術的な実装詳細を定義するものです。これは、以下の2つの上位ドキュメントを具現化するための、開発者向けの直接的な指示書です。

1. **KZN-ARC-DESIGN-001 (アーキテクチャ):** システム全体の「なぜ（Why）」とレイヤー構造（L1〜L4）を定義します。  
2. **KZN-API-SPEC-001 (API仕様):** 公開インターフェース（L1）の「何を（What）」を定義します。

本書（DDD）は、これらのドキュメントの間のギャップを埋め、L1（ファサード）がL2〜L4の内部（pub(crate))モジュールを「どのように（How）」利用してAPI仕様の機能を実現するかを、アルゴリズム、データ構造、エラー処理のレベルで詳述します。

本設計書のすべての実装詳細は、アーキテクチャの基本方針（ARC 序文）、特に「ローカルファースト＆CPUオンリー」（ARC REQ 2.1）および「ゼロ・ランタイム依存」（API 原則 0.2）を達成することを最優先とします。

---

## **4.1 内部モジュール設計: L4 (kaze\_foundation)**

クレート: kaze\_foundation  
責務: 必須技術スタック（REQ 5.0）をラップし、L2（パイプライン）レイヤーに対して、Pure Rustの安定した内部インターフェース（pub(crate))を提供します。

### **4.1.1 NlpService の内部設計 (sudachi.rs / saku)**

kaze\_foundation 内の nlp\_service モジュールとして実装されます。

* **1\. 内部クラス・関数設計:**  
  * pub(crate) struct NlpService: ロード済みの sudachi::Tokenizer インスタンスを（Arc でラップして）保持する構造体。  
  * impl NlpService:  
    * pub(crate) fn new(config: \&EngineConfig) \-\> Result\<Self, KazeNhanhError\>: コンストラクタ。API 0.2 のインメモリ・ロード要件を実装します。  
    * pub(crate) fn tokenize(\&self, text: \&str) \-\> Result\<Vec\<sudachi::Morpheme\>, KazeNhanhError\>: API 2.5 tokenize\_sentence の内部実装。  
  * pub(crate) fn split\_sentences(text: \&str) \-\> Vec\<String\>: saku (ARC 4.1, ) をラップする純粋関数（API 2.5）。kaze\_nhanh::foundation::nlp::split\_sentences として L1 から再エクスポートされます。  
* **2\. データ構造:**  
  * NlpService 構造体は、std::sync::Arc\<sudachi::Tokenizer\> を保持します。Tokenizer 自体はスレッドセーフですが、Arc でラップすることで、KazeNhanhEngine (L1) から L2 パイプラインに参照を安全に渡すことができます。  
* **3\. アルゴリズム・ロジック: NlpService::new (インメモリ・辞書ロード)**  
  * **目的:** API 0.2 および 2.1 の「静的リソース・インジェクション」要件を満たすため、EngineConfig の &'static \[u8\] バイトスライスから sudachi.rs (ARC 4.1, ) をインメモリでロードします。  
  * **疑似コード:**

Rust  
use kaze\_nhanh::{EngineConfig, KazeNhanhError};  
use sudachi::{Config, Dictionary, Tokenizer};  
use sudachi::prelude::SudachiError;  
use std::str::FromStr;  
use std::sync::Arc;

pub(crate) struct NlpService {  
    tokenizer: Arc\<Tokenizer\>,  
}

impl NlpService {  
    // API 2.1 (引用) で示唆される、sudachi v0.6.9 以降の  
    // 組み込みリソースローダーを活用する。  
    pub(crate) fn new(config: \&EngineConfig) \-\> Result\<Self, SudachiError\> {

        // 1\. settings\_bytes (&\[u8\]) を UTF-8 文字列にデコード  
        let settings\_json \= std::str::from\_utf8(config.settings\_bytes)  
           .map\_err(|e| SudachiError::InvalidSettingBytes(e))?;

        // 2\. JSON文字列から Config オブジェクトをパース  
        let mut sudachi\_config \= Config::from\_str(settings\_json)?;

        // 3\. dictionary\_bytes (&\[u8\]) を Config に設定 (API 2.1)  
        //    これがインメモリ・ロードの核心。ファイルシステムへのアクセスが不要になる。  
        sudachi\_config.system\_dict(config.dictionary\_bytes);  
        // (必要に応じて user\_dict も同様に設定)

        // 4\. インメモリ・リソースからTokenizerを構築  
        let tokenizer \= Tokenizer::new(sudachi\_config)?;

        Ok(Self { tokenizer: Arc::new(tokenizer) })  
    }

    pub(crate) fn tokenize(&self, text: &str) \-\> Result\<Vec\<sudachi::Morpheme\>, SudachiError\> {  
        // sudachi::Tokenizer::tokenize は \&self を取り、スレッドセーフである。  
        self.tokenizer.tokenize(text, sudachi::Mode::Normal, false)  
    }  
}

* **4\. エラー処理の詳細:**  
  * NlpService::new で発生した sudachi::Error は、L1の KazeNhanhEngine::new に伝播され、KazeNhanhError::DictionaryLoadError { source: e }（API 2.2）としてラップされます。  
  * NlpService::tokenize で発生した sudachi::Error は、L2経由で L1 に伝播され、KazeNhanhError::TokenizationError { source: e }（API 2.2）としてラップされます。

### **4.1.2 MarkdownService の内部設計 (pulldown-cmark)**

kaze\_foundation 内の markdown\_service モジュールとして実装されます。pulldown-cmark (REQ 5.4, ARC 4.1, ) を利用します。

* **1\. 内部クラス・関数設計:**  
  * 本モジュールはステートレスであり、構造体は不要です。  
  * pub(crate) fn map\_document\_structure(markdown\_text: \&str) \-\> Result\<Vec\<MarkdownSection\>, KazeNhanhError\>: API 2.5 の map\_document\_structure の内部実装。  
* **2\. データ構造:**  
  * kaze\_nhanh::foundation::markdown::MarkdownSection（API 2.5 で定義）を返却します。  
  * **内部ヘルパー:** struct LineOffsetMap { line\_start\_offsets: Vec\<usize\> }: バイトオフセットを行番号に高速変換するための（関数ローカルな）インデックス。  
* **3\. アルゴリズム・ロジック: map\_document\_structure (行番号マッピング)**  
  * **目的:** FR 2.2 の「行番号マッピング」要件を達成します。pulldown-cmark が提供する「バイトオフセット」（API 2.5 洞察 ）を、git diff と相関可能な「物理行番号」（1-based）に正確に変換します。  
  * **疑似コード:**

Rust  
use kaze\_nhanh::foundation::markdown::MarkdownSection;  
use kaze\_nhanh::KazeNhanhError;  
use pulldown\_cmark::{Parser, Event, Tag, HeadingLevel, Options};  
use std::ops::Range;

// バイトオフセットを行番号（1-based）に変換するヘルパー  
struct LineOffsetMap {  
    line\_start\_offsets: Vec\<usize\>,  
    total\_lines: usize,  
}

impl LineOffsetMap {  
    fn new(text: &str) \-\> Self {  
        let mut offsets \= vec\!; // 0行目（1行目）は常にオフセット0  
        offsets.extend(text.match\_indices('\\n').map(|(i, \_)| i \+ 1));  
        let total\_lines \= offsets.len();  
        Self { line\_start\_offsets: offsets, total\_lines }  
    }

    // バイトオフセットから行番号（1-based）を二分探索で特定  
    fn get\_line\_number(&self, byte\_offset: usize) \-\> usize {  
        match self.line\_start\_offsets.binary\_search(\&byte\_offset) {  
            Ok(line\_index) \=\> line\_index \+ 1, // 0-based index to 1-based line  
            Err(insertion\_point) \=\> insertion\_point, // 範囲内 \[0..=N\]、0は1行目を意味する  
        }  
    }  
}

pub(crate) fn map\_document\_structure(  
    markdown\_text: &str  
) \-\> Result\<Vec\<MarkdownSection\>, KazeNhanhError\> {

    let line\_map \= LineOffsetMap::new(markdown\_text);  
    let mut results \= Vec::new();

    // 1\. (ARC 2.3) ソース位置（バイトオフセット）を有効にする  
    let opts \= Options::ENABLE\_SOURCE\_POSITION;  
    let parser \= Parser::new\_ext(markdown\_text, opts);

    // (Event, Range\<usize\>) のタプルを返すイテレータ (API 2.5 )  
    let mut offset\_iter \= parser.into\_offset\_iter();

    // 処理中のセクションを追跡する  
    let mut current\_section: Option\<MarkdownSection\> \= None;

    while let Some((event, offset)) \= offset\_iter.next() {  
        match event {  
            Event::Start(Tag::Heading { level,.. }) \=\> { // (API 2.5 )  
                // 2\. 前のセクションを（あれば）完了させる  
                if let Some(mut section) \= current\_section.take() {  
                    // この見出しの開始が、前のセクションの終了を意味する  
                    let end\_line \= line\_map.get\_line\_number(offset.start);  
                    section.end\_line \= end\_line.saturating\_sub(1); // 前の行  
                    results.push(section);  
                }

                // 3\. 新しいセクションを開始  
                let start\_line \= line\_map.get\_line\_number(offset.start);  
                // (get\_heading\_textは、offset\_iterから次のEvent::Textまでを読むヘルパー)  
                let text \= get\_heading\_text(markdown\_text, &mut offset\_iter, offset); 

                let new\_section \= MarkdownSection {  
                    heading\_text: text,  
                    level: level as u32, // HeadingLevel (H1, H2...) \-\> u32  
                    start\_line,  
                    end\_line: line\_map.total\_lines, // 仮。次の見出しで更新  
                };  
                current\_section \= Some(new\_section);  
            }  
            \_ \=\> {} // 他のイベントは無視  
        }  
    }

    // 4\. 最後のセクションを処理  
    if let Some(mut section) \= current\_section.take() {  
        section.end\_line \= line\_map.total\_lines;  
        results.push(section);  
    }

    Ok(results)  
}

// (get\_heading\_text の実装は省略)

* **4\. エラー処理の詳細:**  
  * KazeNhanhError::MarkdownParseError(String): バイトオフセットのパースや行番号への変換ロジックで予期せぬエラーが発生した場合に生成されます。

### **4.1.3 GitService の内部設計 (git2-rs)**

kaze\_foundation 内の git\_service モジュールとして実装されます。git2-rs (REQ 5.3, ARC 4.1, ) を利用します。

* **1\. 内部クラス・関数設計:**  
  * 本モジュールもステートレスです。  
  * pub(crate) fn get\_markdown\_diffs(options: \&GitReportOptions) \-\> Result\<Vec\<FileDiff\>, KazeNhanhError\>: L2 の GitNativeRAG パイプライン（ARC 2.2）から呼び出されるコア関数。  
* **2\. データ構造:**  
  * pub(crate) struct FileDiff: L2 パイプラインに渡すための、ファイルごとの変更箇所を保持する内部データ構造。  
    Rust  
    \#  
    pub(crate) struct FileDiff {  
        /// リポジトリルートからの相対パス  
        pub path: std::path::PathBuf,  
        /// (行番号, 追加された行のテキスト) のタプル  
        pub added\_lines: Vec\<(usize, String)\>,  
    }

* **3\. アルゴリズム・ロジック: get\_markdown\_diffs (Diff解析)**  
  * **目的:** FR 4.1 および 4.2 を満たします。指定されたリポジトリと期間に基づき、変更された .md ファイルと、追加された行番号（new\_lineno）およびその内容を抽出します。  
  * **疑似コード:**

Rust  
use kaze\_nhanh::{GitReportOptions, KazeNhanhError};  
use git2::{Repository, DiffOptions, DiffFindOptions, Diff, ObjectType, Oid, Time};  
use std::collections::HashMap;  
use std::time::{SystemTime, Duration};  
use std::path::PathBuf;

pub(crate) fn get\_markdown\_diffs(  
    options: \&GitReportOptions  
) \-\> Result\<Vec\<FileDiff\>, KazeNhanhError\> {

    // 1\. リポジトリを開く  
    let repo \= Repository::open(options.repo\_path)  
       .map\_err(|e| KazeNhanhError::RepositoryNotFound(options.repo\_path.to\_string()))?;

    // 2\. 解析対象期間の開始時刻を計算 (FR 4.1)  
    let now \= SystemTime::now().duration\_since(SystemTime::UNIX\_EPOCH)  
       .unwrap\_or\_default().as\_secs() as i64;  
    let since\_secs \= (options.days\_since as i64) \* 86400;  
    let since\_time \= Time::new(now \- since\_secs, 0);

    // 3\. HEADからコミット履歴をウォーク  
    let mut revwalk \= repo.revwalk()?;  
    revwalk.push\_head()?;

    // 変更をファイルパスごとに集約する  
    let mut diffs\_map: HashMap\<PathBuf, Vec\<(usize, String)\>\> \= HashMap::new();  
    let target\_exts \= options.target\_extensions.clone().unwrap\_or(vec\!\[".md", ".markdown"\]);

    for oid in revwalk {  
        let oid \= oid?;  
        let commit \= repo.find\_commit(oid)?;

        // 4\. 指定期間より古いコミットに達したら停止  
        if commit.time().seconds() \< since\_time.seconds() {  
            break;  
        }

        // 5\. 親コミットとの差分(Diff)を取得 (API 2.3 )  
        if commit.parent\_count() \== 0 { continue; } // 初回コミットはスキップ  
        let parent \= commit.parent(0)?;  
        let parent\_tree \= parent.tree()?;  
        let commit\_tree \= commit.tree()?;

        let mut diff\_opts \= DiffOptions::new();  
        // パススペックで対象を絞る  
        for ext in \&target\_exts {  
            diff\_opts.pathspec(format\!("\*{}", ext));  
        }

        let diff \= repo.diff\_tree\_to\_tree(  
            Some(\&parent\_tree), Some(\&commit\_tree), Some(&mut diff\_opts)  
        )?;

        // 6\. 差分を解析 (FR 4.2)  
        diff.foreach(  
            &mut |delta, \_| { true }, // File callback (常に続行)  
            None, // Binary callback  
            Some(&mut |delta, hunk, line| { // Hunk/Line callback (API 2.3 , )  
                // 7\. (FR 4.2) 追加された行 (+) のみを取得  
                if line.origin() \== '+' |

| line.origin() \== ' ' { // Context行も含む場合がある  
if line.origin()\!= '+' { return true; } // 追加行のみを対象

                    if let Some(path) \= delta.new\_file().path() {  
                        let line\_num \= line.new\_lineno().unwrap\_or(0) as usize;  
                        if line\_num \== 0 { return true; } // 行番号が取れない場合はスキップ  
                          
                        let content \= std::str::from\_utf8(line.content())  
                           .unwrap\_or("").trim\_start\_matches('+').to\_string();  
                          
                        diffs\_map.entry(path.to\_path\_buf())  
                                .or\_default()  
                                .push((line\_num, content));  
                    }  
                }  
                true // continue  
            }),  
            None, // Data callback  
        )?;  
    }

    // 8\. HashMap を Vec\<FileDiff\> に変換  
    let results \= diffs\_map.into\_iter()  
       .map(|(path, added\_lines)| FileDiff { path, added\_lines })  
       .collect();

    Ok(results)  
}  
\`\`\`

* **4\. エラー処理の詳細:**  
  * Repository::open の失敗は KazeNhanhError::RepositoryNotFound にマッピングされます。  
  * その他の git2::Error は、KazeNhanhError::GitOperationError（API 2.2）として L1 に伝播されます（? 演算子と \#\[from\] による）。

---

## **4.2 内部モジュール設計: L3 (kaze\_inference)**

クレート: kaze\_inference  
責務: AIモデルのロードと実行（FR 3.3.2）に関するすべての複雑性をカプセル化します。

### **4.2.1 InferenceEngine の設計 (モデル保持)**

アーキテクチャ（ARC 1.2）の「Singleton」要求と、API（API 0.1, 2.1）の「Tauriステート管理」要求の**両立**を図るため、以下の設計を採用します。

* **設計決定:** グローバルな lazy\_static（ARC 1.2）は採用**しません**。  
* **理由:** API設計書（API 0.1）は、KazeNhanhEngine が tauri::Builder::manage() で管理されることを明示的に要求しています。この KazeNhanhEngine インスタンス（Tauriステート）が、アプリケーションのライフサイクル全体における事実上の「Singleton」として機能します。  
* **実装:** kaze\_inference クレートは、KazeNhanhEngine (L1) によって**所有される** InferenceEngine 構造体を定義します。この InferenceEngine がロード済みの candle (ARC 4.1, ) モデルを保持します。L1 は Arc\<Mutex\<InferenceEngine\>\>（API 2.1）の形でこれを保持し、Tauriの Send \+ Sync 要件を満たします。  
* **1\. 内部クラス・関数設計:**  
  * pub(crate) trait KazeModel: Send \+ Sync: candle の具体的なモデル（Llama, Phi-3等）を抽象化する内部トレイト。  
    * fn synthesize(\&mut self, prompt: String) \-\> Result\<String, candle\_core::Error\>;  
  * pub(crate) struct InferenceEngine:  
    * model: Box\<dyn KazeModel\>  
    * tokenizer: Arc\<Mutex\<tokenizers::Tokenizer\>\> (candle が使用する tokenizers クレート)  
  * impl InferenceEngine:  
    * pub(crate) fn new(model\_bytes: &'static \[u8\]) \-\> Result\<Self, KazeNhanhError\>: コンストラクタ。  
    * pub(crate) fn synthesize(\&mut self, prompt: String) \-\> Result\<String, KazeNhanhError\>: KazeModel の synthesize を呼び出すラッパー。  
* **2\. データ構造:**  
  * InferenceEngine が、Box\<dyn KazeModel\> と Arc\<Mutex\<Tokenizer\>\> を保持します。  
* **3\. アルゴリズム・ロジック: InferenceEngine::new (GGUFインメモリ・ロード)**  
  * **目的:** API 0.2 および 2.1 の最重要要件。「ゼロセットアップ」のため、EngineConfig の model\_bytes から candle モデルをインメモリでロードします。  
  * **疑似コード:**

Rust  
use kaze\_nhanh::KazeNhanhError;  
use candle\_core::{Device, Error as CandleError, DType};  
use candle\_transformers::models::quantized\_llama as model; // 例: Llama  
use candle\_transformers::generation::LogitsProcessor;  
use tokenizers::Tokenizer;  
use std::io::Cursor;  
use std::sync::{Arc, Mutex};  
use candle\_transformers::quantized\_var\_builder::VarBuilder;

// (KazeModel トレイトの定義)...

pub(crate) struct InferenceEngine {  
    model: Box\<dyn KazeModel\>,  
    // (tokenizer は KazeModel ラッパー内に移動)  
}

// Llama を KazeModel に適合させるラッパー  
struct LlamaModelWrapper {  
    model: model::Model,  
    tokenizer: Arc\<Mutex\<Tokenizer\>\>,  
    logits\_processor: LogitsProcessor,  
    //...  
}

impl KazeModel for LlamaModelWrapper {  
    fn synthesize(&mut self, prompt: String) \-\> Result\<String, CandleError\> {  
        //... (4.2.2 を参照)...  
    }  
}

impl InferenceEngine {  
    pub(crate) fn new(  
        model\_bytes: &'static \[u8\]  
    ) \-\> Result\<Self, KazeNhanhError\> {

        // 1\. (ARC 3.1.1) CPU-Only (NFR 1.1) を強制  
        let device \= Device::Cpu;

        // 2\. (API 2.1 洞察) バイトスライスを Cursor でラップ  
        let mut cursor \= Cursor::new(model\_bytes);

        // 3\. GGUFファイルリーダーを初期化  
        let gguf\_content \= gguf\_file::Content::read(&mut cursor)  
           .map\_err(|e| KazeNhanhError::ModelLoadError { source: e.into() })?;

        // 4\. トークナイザーをGGUFメタデータからロード (API 2.1 )  
        let tokenizer \= build\_tokenizer\_from\_gguf(\&gguf\_content)?;

        // 5\. GGUFから candle モデルをビルド (API 2.1 )  
        let vb \= VarBuilder::from\_gguf\_decompressed(&mut cursor, \&gguf\_content, \&device)?;

        let config \= model::Config::from\_gguf(\&gguf\_content)?;  
        let model \= model::Model::load(vb, config, DType::F32)?; // 量子化モデルでも内部は F32 でロード

        // LogitsProcessor (サンプリング戦略) の初期化  
        let logits\_processor \= LogitsProcessor::new(299792, None, None); // (seed, temp, top\_p)

        let wrapped\_model \= LlamaModelWrapper {  
            model,  
            tokenizer: Arc::new(Mutex::new(tokenizer)),  
            logits\_processor,  
        };

        Ok(Self {  
            model: Box::new(wrapped\_model),  
        })  
    }

    // L3 内部の synthesize 呼び出し  
    pub(crate) fn synthesize(&mut self, prompt: String) \-\> Result\<String, KazeNhanhError\> {  
        self.model.synthesize(prompt)  
           .map\_err(|e| KazeNhanhError::ModelInferenceError { source: e })  
    }  
}  
// (build\_tokenizer\_from\_gguf ヘルパーの実装は省略)

* **4\. エラー処理の詳細:**  
  * gguf\_file::Error または candle\_core::Error が発生した場合、KazeNhanhError::ModelLoadError（API 2.2）にマッピングされます。

### **4.2.2 synthesize メソッドの内部ロジック (KazeModel 実装)**

* **3\. アルゴリズム・ロジック:**  
  * **目的:** KazeModel トレイトの synthesize メソッドの実装。L2から渡されたプロンプト（例：GitNativeRAG の相関結果、HybridSummarizer の重要文）に基づき、テキストを生成します。  
  * **疑似コード (LlamaModelWrapper 内):**

Rust  
impl KazeModel for LlamaModelWrapper {  
    fn synthesize(&mut self, prompt: String) \-\> Result\<String, CandleError\> {

        let mut tokenizer \= self.tokenizer.lock().unwrap();  
        let tokens \= tokenizer.encode(prompt, true)?.get\_ids().to\_vec();

        let mut generated\_tokens \= Vec::new();  
        let mut all\_tokens \= tokens;  
        let mut result\_text \= String::new();  
        let mut last\_decoded\_len \= 0;

        const MAX\_TOKENS: usize \= 512; // 生成トークン数の上限  
        const MAX\_CONTEXT: usize \= 2048; // モデルへの入力長

        for i in 0..MAX\_TOKENS {  
            let context\_start \= all\_tokens.len().saturating\_sub(MAX\_CONTEXT);  
            let context \= \&all\_tokens\[context\_start..\];

            // CPU 推論の実行 (ARC 3.1.1)  
            let logits \= self.model.forward(\&context, all\_tokens.len() \- context.len())?;

            let next\_token \= self.logits\_processor.sample(\&logits)?;

            if next\_token \== tokenizer.eos\_token\_id().unwrap\_or(0) { break; }

            all\_tokens.push(next\_token);  
            generated\_tokens.push(next\_token);

            // ストリーミング・デコード（部分的なUTF-8を処理）  
            if let Ok(decoded) \= tokenizer.decode(\&generated\_tokens, false) {  
                if decoded.len() \> last\_decoded\_len {  
                     result\_text.push\_str(\&decoded\[last\_decoded\_len..\]);  
                     last\_decoded\_len \= decoded.len();  
                }  
            }  
        }

        Ok(result\_text)  
    }  
}

* **4\. エラー処理の詳細:**  
  * model.forward() や tokenizer.decode() で発生した candle\_core::Error は、InferenceEngine::synthesize に伝播され、KazeNhanhError::ModelInferenceError（API 2.2）にマッピングされます。

---

## **4.3 内部モジュール設計: L2 (kaze\_pipeline)**

クレート: kaze\_pipeline  
責務: L3（推論）とL4（基盤）のサービスをオーケストレーションし、主要なビジネスロジック（FR 3, FR 4）を実行します。

### **4.3.1 HybridSummarizer (FR 3 "Drip and Stir") パイプライン**

* **1\. 内部クラス・関数設計:**  
  * pub(crate) struct HybridSummarizer\<'a\>: L1からL3/L4への参照を保持します。  
    * nlp\_service: &'a Arc\<NlpService\> (L4)  
    * inference\_engine: &'a Arc\<Mutex\<InferenceEngine\>\> (L3)  
  * impl\<'a\> HybridSummarizer\<'a\>:  
    * pub(crate) fn execute(\&self, text: \&str, count: usize) \-\> Result\<String, KazeNhanhError\>: API summarize\_document の内部実装。  
    * pub(crate) fn extract\_only(\&self, text: \&str, count: usize) \-\> Result\<Vec\<String\>, KazeNhanhError\>: API extract\_important\_sentences の内部実装 (実体は 4.3.2)。  
* **3\. アルゴリズム・ロジック: execute (オーケストレーション)**  
  * **目的:** ARC 2.1 で定義された "Drip and Stir" パイプラインを実行します。  
  * **フローチャート / 疑似コード:**

Rust  
// In kaze\_pipeline::hybrid\_summarizer

pub(crate) fn execute(&self, text: &str, count: usize) \-\> Result\<String, KazeNhanhError\> {

    // \--- STAGE 1: 抽出 (Drip) (FR 3.3.1) \---  
    // 1\. LexRankEngine を呼び出し、重要文を取得 (4.3.2 を参照)  
    let important\_sentences \= self.extract\_only(text, count)?;

    if important\_sentences.is\_empty() {  
        return Ok("".to\_string()); // 要約対象なし  
    }

    // \--- STAGE 2: 合成 (Stir) (FR 3.3.2) \---  
    // 2\. L3 (InferenceEngine) へのプロンプトを作成  
    let prompt \= Self::build\_synthesis\_prompt(\&important\_sentences);

    // 3\. L3 (InferenceEngine) を呼び出す (ARC 2.1)  
    let mut engine \= self.inference\_engine.lock()  
       .map\_err(|\_| KazeNhanhError::InvalidInput("Failed to lock inference engine".to\_string()))?;

    let summary \= engine.synthesize(prompt)?; // KazeNhanhError::ModelInferenceError

    Ok(summary)  
}

fn build\_synthesis\_prompt(sentences: &Vec\<String\>) \-\> String {  
    // 例: 「以下の文を自然な日本語の段落として再構成してください: \[文1\]\[文2\]...」  
    format\!(  
        "以下の主要な文を、文脈を補いながら自然な日本語のパラグラフとして再構成し、要約を作成してください。\\n\\n{}"  
        , sentences.join("\\n")  
    )  
}

### **4.3.2 アルゴリズム: LexRankEngine (FR 3.3.1) の自前実装**

kaze\_pipeline 内の lex\_rank\_engine モジュールとして実装されます。ARC 4.2 の「自前実装」の決定に従います。

* **1\. 内部クラス・関数設計:**  
  * pub(crate) mod lex\_rank\_engine: ステートレスなモジュール。  
  * pub(crate) fn rank\_sentences(nlp: \&Arc\<NlpService\>, text: \&str, count: usize) \-\> Result\<Vec\<String\>, KazeNhanhError\>: extract\_only から呼び出されるコア関数。  
* **2\. データ構造:**  
  * struct SentenceNode { text: String, vector: HashMap\<String, u32\> }: 文と、その文のBag-of-Words（名詞・動詞のみ）ベクトル。  
* **3\. アルゴリズム・ロジック: rank\_sentences (LexRank)**  
  * **目的:** ARC 4.2 の方針に基づき、sudachi.rs (L4) と petgraph (API 2.5 ) などのグラフライブラリを組み合わせて LexRank を実装します。  
  * **疑似コード:**

Rust  
use kaze\_foundation::nlp\_service::NlpService;  
use std::collections::HashMap;  
use ndarray::{Array, Array2, Axis}; // (ARC 4.2  で示唆)  
use petgraph::graph::UnGraph; // 無向グラフ  
use petgraph::algo::pagerank\_weighted;  
use std::sync::Arc;

// (SentenceNode の定義)

pub(crate) fn rank\_sentences(  
    nlp: \&Arc\<NlpService\>, text: &str, count: usize  
) \-\> Result\<Vec\<String\>, KazeNhanhError\> {

    // 1\. L4: NlpService (saku) で文に分割  
    let sentence\_strings \= kaze\_foundation::nlp\_service::split\_sentences(text);  
    if sentence\_strings.len() \<= count { return Ok(sentence\_strings); }

    // 2\. L4: NlpService (sudachi) で各文を形態素解析し、BoWベクトル作成  
    let mut nodes: Vec\<SentenceNode\> \= Vec::new();  
    for s in \&sentence\_strings {  
        let morphemes \= nlp.tokenize(s)  
           .map\_err(|e| KazeNhanhError::TokenizationError { source: e })?;

        let mut vector: HashMap\<String, u32\> \= HashMap::new();  
        for m in morphemes {  
            // 名詞、動詞、形容詞のみをカウント（例）  
            let pos \= m.part\_of\_speech().get(0).unwrap\_or("");  
            if pos \== "名詞" |

| pos \== "動詞" |  
| pos \== "形容詞" {  
\*vector.entry(m.dictionary\_form()).or\_insert(0) \+= 1;  
}  
}  
nodes.push(SentenceNode { text: s.clone(), vector });  
}

    // 3\. TF-IDF または コサイン類似度マトリックス (N x N) を構築  
    //    (ここでは簡略化のためコサイン類似度を使用) (API 2.5 )  
    let n \= nodes.len();  
    let mut similarity\_matrix \= Array2::\<f64\>::zeros((n, n));  
    for i in 0..n {  
        for j in (i \+ 1)..n { // 対角と半分を計算  
            let sim \= calculate\_cosine\_similarity(\&nodes\[i\].vector, \&nodes\[j\].vector);  
            similarity\_matrix\[\[i, j\]\] \= sim;  
            similarity\_matrix\[\[j, i\]\] \= sim;  
        }  
    }

    // 4\. 類似度グラフを構築し、PageRank を実行 (ARC 4.2 引用)  
    let mut graph \= UnGraph::\<usize, f64\>::new\_undirected();  
    let node\_indices: Vec\<\_\> \= (0..n).map(|i| graph.add\_node(i)).collect();  
    for i in 0..n {  
        for j in (i \+ 1)..n {  
            let weight \= similarity\_matrix\[\[i, j\]\];  
            if weight \> 0.01 { // 閾値  
                graph.add\_edge(node\_indices\[i\], node\_indices\[j\], weight);  
            }  
        }  
    }  
      
    let pagerank\_scores \= pagerank\_weighted(\&graph, 0.85); // 0.85は標準的なdamping factor

    // 5\. スコアでソートし、上位 \`count\` 件の文のインデックスを取得  
    let mut ranked\_indices: Vec\<usize\> \= (0..n).collect();  
    ranked\_indices.sort\_by(|\&a, \&b|   
        pagerank\_scores\[node\_indices\[a\]\].partial\_cmp(\&pagerank\_scores\[node\_indices\[b\]\]).unwrap().reverse()  
    );

    // 6\. 元の出現順に戻す（要約の可読性のため）  
    let mut top\_indices: Vec\<usize\> \= ranked\_indices.into\_iter().take(count).collect();  
    top\_indices.sort();  
      
    let result \= top\_indices.into\_iter().map(|i| nodes\[i\].text.clone()).collect();  
    Ok(result)  
}

// (calculate\_cosine\_similarity ヘルパーの実装は省略)  
\`\`\`

* **4\. エラー処理の詳細:**  
  * L4 NlpService からの TokenizationError を伝播します。  
  * グラフ計算や行列計算の失敗は KazeNhanhError::SummarizeEngineError(String)（API 2.2）として報告されます。

### **4.3.3 GitNativeRAG (FR 4\) パイプライン**

* **1\. 内部クラス・関数設計:**  
  * pub(crate) struct GitNativeRAG\<'a\>:  
    * inference\_engine: &'a Arc\<Mutex\<InferenceEngine\>\> (L3)  
    * // L4 サービス (Git, Markdown) はステートレスなため、参照は不要。  
  * impl\<'a\> GitNativeRAG\<'a\>:  
    * pub(crate) fn execute(\&self, options: \&GitReportOptions) \-\> Result\<String, KazeNhanhError\>: API generate\_git\_report の内部実装。  
* **3\. アルゴリズム・ロジック: execute (オーケストレーション)**  
  * **目的:** ARC 2.2 で定義された GitネイティブRAG の5ステップ・パイプラインを実行します。  
  * **フローチャート / 疑似コード:**

Rust  
use kaze\_foundation::{git\_service, markdown\_service};  
use kaze\_pipeline::correlation\_engine;  
use std::path::Path;

pub(crate) fn execute(  
    &self, options: \&GitReportOptions  
) \-\> Result\<String, KazeNhanhError\> {

    // \--- ステップ 1 & 2: 差分の検出と特定 (FR 4.1, 4.2) \---  
    // 1\. L4: GitService を呼び出し、FileDiff リストを取得  
    let file\_diffs \= git\_service::get\_markdown\_diffs(options)?; // (4.1.3 を参照)

    let mut all\_contextual\_changes: Vec\<correlation\_engine::ContextualChange\> \= Vec::new();  
    let repo\_root \= Path::new(options.repo\_path);

    // \--- ステップ 3 & 4: 文脈の特定と相関 (FR 4.3, 4.4) \---  
    for diff in file\_diffs {  
        // 2\. ファイルのフルパスを取得  
        let full\_path \= repo\_root.join(\&diff.path);

        // 3\. L4: MarkdownService のために対象ファイルの内容を読み込む  
        let markdown\_content \= match std::fs::read\_to\_string(\&full\_path) {  
            Ok(content) \=\> content,  
            Err(e) \=\> {  
                // ファイルが削除された、またはアクセスできない。このDiffはスキップ  
                continue;   
            }  
        };

        // 4\. L4: MarkdownService を呼び出し、構造マップを取得  
        let sections \= match markdown\_service::map\_document\_structure(\&markdown\_content) { // (4.1.2 を参照)  
             Ok(s) \=\> s,  
             Err(e) \=\> {  
                 // Markdown のパース失敗。このファイルはスキップ  
                 continue;  
             }  
        };

        // 5\. 相関ロジックを実行 (4.3.4 を参照)  
        let changes \= correlation\_engine::correlate(\&diff, \&sections);  
        all\_contextual\_changes.extend(changes);  
    }

    if all\_contextual\_changes.is\_empty() {  
        return Ok("指定された期間に関連するMarkdownの変更はありませんでした。".to\_string());  
    }

    // \--- ステップ 5: レポート生成 (FR 4.5) \---  
    // 6\. L3 (InferenceEngine) へのプロンプトを作成 (REQ 3.4.5 形式)  
    let prompt \= Self::build\_rag\_prompt(options, \&all\_contextual\_changes);

    // 7\. L3 (InferenceEngine) を呼び出す (ARC 2.2 \[重要\])  
    let mut engine \= self.inference\_engine.lock()  
       .map\_err(|\_| KazeNhanhError::InvalidInput("Failed to lock inference engine".to\_string()))?;

    let report \= engine.synthesize(prompt)?; // KazeNhanhError::ModelInferenceError

    Ok(report)  
}

fn build\_rag\_prompt(  
    options: \&GitReportOptions, changes: &Vec\<correlation\_engine::ContextualChange\>  
) \-\> String {  
    // (REQ 3.4.5 の形式に準拠したプロンプトを構築するロジック)  
    let mut prompt \= format\!(  
        "{}日前からのGitリポジトリの変更履歴に基づき、進捗レポートを作成してください。\\n\\n",  
        options.days\_since  
    );  
    prompt.push\_str("検出された変更点:\\n");

    for change in changes {  
        prompt.push\_str(&format\!(  
            "- ファイル: {}\\n  \- セクション: {}\\n  \- 追加行 (L{}): {}\\n",  
            change.file\_path.display(),  
            change.section\_heading,  
            change.line\_number,  
            change.added\_line\_text.trim()  
        ));  
    }

    prompt.push\_str("\\n以上の変更を要約し、自然な日本語のレポートを作成してください。");  
    prompt  
}

### **4.3.4 アルゴリズム: CorrelationEngine (FR 4.4 の相関ロジック)**

kaze\_pipeline 内の correlation\_engine モジュールとして実装されます。

* **1\. 内部クラス・関数設計:**  
  * pub(crate) mod correlation\_engine: ステートレスなモジュール。  
  * pub(crate) fn correlate(diff: \&FileDiff, sections: &) \-\> Vec\<ContextualChange\>:  
* **2\. データ構造:**  
  * pub(crate) struct ContextualChange: L2 GitNativeRAG が L3 InferenceEngine へのプロンプトを作成するために使用する内部データ構造。  
    Rust  
    \#  
    pub(crate) struct ContextualChange {  
        /// 変更が検出されたファイルパス  
        pub file\_path: std::path::PathBuf,  
        /// 変更が属するセクションの見出し (例: "\#\# 今週のタスク")  
        pub section\_heading: String,  
        /// 追加された行のテキスト  
        pub added\_line\_text: String,  
        /// 変更箇所の行番号  
        pub line\_number: usize,  
    }

* **3\. アルゴリズム・ロジック: correlate (行番号の突合)**  
  * **目的:** L4:GitService からの「変更行番号」（FileDiff）と、L4:MarkdownService からの「セクションの行範囲」（MarkdownSection）を突合させ、変更の「文脈」を特定します。  
  * **疑似コード:**

Rust  
use kaze\_foundation::git\_service::FileDiff;  
use kaze\_nhanh::foundation::markdown::MarkdownSection;

pub(crate) fn correlate(  
    diff: \&FileDiff,  
    sections: &  
) \-\> Vec\<ContextualChange\> {

    let mut results \= Vec::new();  
    let default\_heading \= "（見出しなし）".to\_string();

    // 1\. \`FileDiff\` 内の「追加された行」をイテレート  
    for (line\_num, line\_text) in \&diff.added\_lines {

        // 2\. この行番号が属するセクションを \`MarkdownSection\` リストから検索  
        //    (sections リストがソートされている前提)  
        let mut found\_heading \= \&default\_heading;

        // セクションは（通常）ソートされているため、\`binary\_search\_by\` が効率的だが、  
        // 重複やネストを考慮し、find を使用する  
        let found\_section \= sections.iter().find(|section| {  
            // 3\. FR 4.4 のコアロジック:  
            //    line\_num が section.start\_line と end\_line の範囲内か判定  
            \*line\_num \>= section.start\_line && \*line\_num \<= section.end\_line  
        });

        if let Some(section) \= found\_section {  
             found\_heading \= \&section.heading\_text;  
        }

        results.push(ContextualChange {  
            file\_path: diff.path.clone(),  
            section\_heading: found\_heading.clone(),  
            added\_line\_text: line\_text.clone(),  
            line\_number: \*line\_num,  
        });  
    }

    results  
}  
---

## **4.4 L1 (kaze\_facade) 実装設計**

クレート: kaze\_facade (またはライブラリのルートクレート kaze\_nhanh)  
責務: API設計書（API 1.4）で定義された pub インターフェースを実装します。L2〜L4の内部モジュールを呼び出し、オーケストレーションし、統一された KazeNhanhError を返します。

### **4.4.1 KazeNhanhEngine::new (コンストラクタ) の実装**

* **3\. アルゴリズム・ロジック:**  
  * **目的:** API 2.1 の実装。EngineConfig を受け取り、L3（推論）とL4（NLP）のステートフルなサービスを初期化し、Tauriステート（API 0.1）として管理可能な Self インスタンスを構築します。  
  * **疑似コード:**

Rust  
// In kaze\_nhanh (root crate)

use kaze\_foundation::nlp\_service::NlpService;  
use kaze\_inference::InferenceEngine;  
use std::sync::{Arc, Mutex};  
use crate::{EngineConfig, KazeNhanhError}; // (kaze\_nhanh クレート内)

// API 2.1 で定義された公開 struct  
pub struct KazeNhanhEngine {  
    // L3: 推論エンジン (Mutex でスレッドセーフ) (API 2.1)  
    internal\_model: Arc\<Mutex\<InferenceEngine\>\>,

    // L4: NLP サービス (Sudachi)  
    internal\_nlp\_service: Arc\<NlpService\>,  
}

// API 1.4, 2.1 で定義された公開コンストラクタ  
impl KazeNhanhEngine {  
    pub fn new(config: EngineConfig) \-\> Result\<Self, KazeNhanhError\> {

        // 1\. L4 (NlpService) の初期化 (4.1.1 を参照)  
        let nlp\_service \= NlpService::new(\&config)  
           .map\_err(|e| KazeNhanhError::DictionaryLoadError { source: e })?;

        // 2\. L3 (InferenceEngine) の初期化 (4.2.1 を参照)  
        let inference\_engine \= InferenceEngine::new(config.model\_bytes)?;  
        // (InferenceEngine::new は KazeNhanhError::ModelLoadError を直接返す)

        // 3\. エンジンを Arc/Mutex でラップして保持  
        Ok(Self {  
            internal\_model: Arc::new(Mutex::new(inference\_engine)),  
            internal\_nlp\_service: Arc::new(nlp\_service),  
        })  
    }

    //... (他の pub fn の実装)...  
}

### **4.4.2 generate\_git\_report の実装フロー**

* **3\. アルゴリズム・ロジック:**  
  * **目的:** API 2.3 の実装。L2 の GitNativeRAG パイプラインをキックします。  
  * **疑似コード:**

Rust  
// impl KazeNhanhEngine

pub fn generate\_git\_report(  
    &self, options: GitReportOptions  
) \-\> Result\<String, KazeNhanhError\> {

    // 1\. L2 パイプラインを初期化  
    let rag\_pipeline \= kaze\_pipeline::GitNativeRAG {  
        inference\_engine: &self.internal\_model, // L3 の参照を渡す  
    };

    // 2\. L2 パイプラインを実行 (4.3.3 を参照)  
    // L2 は内部で L4 (git\_service, markdown\_service) を呼び出す  
    rag\_pipeline.execute(\&options)  
}

* **4\. エラー処理の詳細:**  
  * rag\_pipeline.execute() が返す KazeNhanhError（GitOperationError, MarkdownParseError, ModelInferenceError, RepositoryNotFound 等）は、? 演算子によって呼び出し元（Tauriコマンド）にそのまま伝播されます。

### **4.4.3 summarize\_document の実装フロー**

* **3\. アルゴリズム・ロジック:**  
  * **目的:** API 2.4 の実装。L2 の HybridSummarizer パイプラインをキックします。  
  * **疑似コード:**

Rust  
// impl KazeNhanhEngine

pub fn summarize\_document(  
    &self, text: &str  
) \-\> Result\<String, KazeNhanhError\> {

    const DEFAULT\_SUMMARY\_SENTENCES: usize \= 5; // 設定可能にしてもよい

    // 1\. L2 パイプラインを初期化  
    let summarizer \= kaze\_pipeline::HybridSummarizer {  
        nlp\_service: &self.internal\_nlp\_service, // L4 の参照を渡す  
        inference\_engine: &self.internal\_model,  // L3 の参照を渡す  
    };

    // 2\. L2 パイプラインを実行 (4.3.1 を参照)  
    summarizer.execute(text, DEFAULT\_SUMMARY\_SENTENCES)  
}

### **4.4.4 extract\_important\_sentences の実装フロー**

* **3\. アルゴリズム・ロジック:**  
  * **目的:** API 2.5 の実装。L2 の LexRankEngine のみ（"Drip" ステージのみ）を実行します。  
  * **疑似コード:**

Rust  
// impl KazeNhanhEngine

pub fn extract\_important\_sentences(  
    &self, text: &str, count: usize  
) \-\> Result\<Vec\<String\>, KazeNhanhError\> {

    // 1\. L2 の LexRankEngine を直接呼び出す (4.3.2 を参照)  
    kaze\_pipeline::lex\_rank\_engine::rank\_sentences(  
        &self.internal\_nlp\_service, // L4 の参照を渡す  
        text,  
        count  
    )  
}  
---

## **4.5 内部データ構造定義 (集約)**

開発者の参照のため、レイヤー間（L4 \-\> L2、L2 \-\> L3プロンプト）で渡される主要な pub(crate) データ構造を一覧化します。

**表 4.1: 主要な内部データ構造一覧**

| データ構造 | 定義クレート | 生成元 (L4) | 消費先 (L2) | 目的 (FR) |
| :---- | :---- | :---- | :---- | :---- |
| FileDiff | kaze\_foundation | git\_service (4.1.3) | GitNativeRAG (4.3.3) | FR 4.2: 変更された行番号とテキストをL2に渡す。 |
| ContextualChange | kaze\_pipeline | correlation\_engine (4.3.4) | GitNativeRAG (4.3.3) | FR 4.4: L3へのプロンプト生成の元となる、文脈化された変更履歴。 |
| SentenceNode | kaze\_pipeline | lex\_rank\_engine (4.3.2) | lex\_rank\_engine (4.3.2) | FR 3.3.1: LexRank計算のため、文とBoWベクトルを保持する。 |

---

## **4.6 内部エラー処理フロー (集約)**

API設計書（API 2.2, 表1）を補完し、基盤ライブラリのエラーがL1の KazeNhanhError にどのように伝播・変換されるかを定義します。

**表 4.2: 詳細エラー伝播パス**

| 発生源ライブラリ | 基盤エラー | L4 (kaze\_foundation) | L3 (kaze\_inference) | L2 (kaze\_pipeline) | L1 (kaze\_facade) | 公開エラー (KazeNhanhError) |
| :---- | :---- | :---- | :---- | :---- | :---- | :---- |
| git2-rs | git2::Error (Open) | git\_service::get\_markdown\_diffs | \- | GitNativeRAG::execute \-\> ? | generate\_git\_report \-\> ? | RepositoryNotFound |
| git2-rs | git2::Error (Other) | git\_service::get\_markdown\_diffs | \- | GitNativeRAG::execute \-\> ? | generate\_git\_report \-\> ? | GitOperationError |
| candle\_core | candle\_core::Error | \- | InferenceEngine::new \-\> Err | \- | KazeNhanhEngine::new \-\> Err | ModelLoadError |
| gguf\_file | gguf\_file::Error | \- | InferenceEngine::new \-\> Err | \- | KazeNhanhEngine::new \-\> Err | ModelLoadError |
| candle\_core | candle\_core::Error | \- | InferenceEngine::synthesize \-\> Err | GitNativeRAG / HybridSummarizer \-\> ? | generate\_git\_report / summarize\_document \-\> ? | ModelInferenceError |
| sudachi.rs | sudachi::Error (Load) | NlpService::new \-\> Err | \- | \- | KazeNhanhEngine::new \-\> Err | DictionaryLoadError |
| sudachi.rs | sudachi::Error (Tokenize) | NlpService::tokenize \-\> Err | \- | HybridSummarizer / lex\_rank\_engine \-\> ? | extract\_important\_sentences \-\> ? | TokenizationError |
| pulldown-cmark | (内部ロジック) | markdown\_service::map\_document\_structure \-\> Err | \- | GitNativeRAG::execute \-\> ? | generate\_git\_report \-\> ? | MarkdownParseError |
| (L2ロジック) | (内部ロジック) | \- | \- | lex\_rank\_engine::rank\_sentences \-\> Err | extract\_important\_sentences \-\> ? | SummarizeEngineError |

| std::fs | std::io::Error | \- | \- | GitNativeRAG::execute (read file) | generate\_git\_report \-\> ? | IoError |