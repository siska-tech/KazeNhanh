# 開発PCのセットアップ

対象: Windows PowerShell 5.1以上。LinuxのCIではPowerShell 7を利用する。

## Windowsの初回準備

1. [Rustup](https://rustup.rs)でstableのMSVC toolchainを導入する。
2. Visual StudioまたはBuild Toolsで「C++によるデスクトップ開発」、MSVC x64/x86とWindows SDKを導入する。
3. Gitを導入する。

必要なtoolchainを新しく入れる場合の例:

```powershell
winget install --id Rustlang.Rustup --exact
winget install --id Microsoft.VisualStudio.2022.BuildTools --exact --override '--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended'
winget install --id Git.Git --exact
```

既に入っていれば再インストールは不要。セットアップスクリプトはCargoをPATH、CARGO_HOME、ユーザーの.cargo/binの順で探す。VSのC++ツールも検出する。OS設定やユーザーPATHを恒久変更せず、不足があれば手順付きで停止する。VS検出はSDK/linkerの動作保証ではなく、下記の検証でnative buildを確認する。

リポジトリ直下から実行:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev/setup-dev.ps1 -Verify
```

`-ExecutionPolicy Bypass`はこのプロセスだけに適用する。組織ポリシーが実行を制限するPCではそのポリシーに従う。PowerShell 7の場合は`pwsh -NoProfile -File scripts/dev/setup-dev.ps1 -Verify`でも実行できる。

- `-AllDictionaries`: 固定版small/core/fullを揃える。通常はsmallのみ。比較手順は[辞書matrix](sudachi-dictionary-matrix.md)。
- `-CheckOnly`: Cargo/Git/MSVCの存在確認。downloadしない。
- `-Offline`: 取得済みCargo依存と辞書archiveだけを使う。初回には利用不可。
- `-Verify`: 準備後にformat、本番build、実辞書smoke、テスト登録確認、モック推論のテストを実行。

セットアップと検証を分ける場合:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev/setup-dev.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev/verify.ps1 -Offline
```

## Sudachi資産

RustのSudachi v0.6.9はCargo.lockで固定する。辞書は公式配布の[SudachiDict-small 20250129](https://pypi.org/project/SudachiDict-small/20250129/)を使用する。これは再現用fixtureであり、最新辞書や全用途向けの最適辞書という意味ではない。

`resources/sudachi/dictionary.lock.json`にURL・version・archiveと抽出辞書のSHA256を記録。公式wheelをZIPとして読み、system.dicとライセンスだけを取り出す。PythonやSudachiPyのインストール、パッケージコードの実行は不要。

- archive cache: `target/dev-assets/`
- 辞書: `resources/sudachi/system.dic`（Git管理外）
- ライセンス: `resources/sudachi/licenses/`（配布元の文書を保存）
- 設定: `resources/sudachi/sudachi.json`（Git管理対象）

再実行ではcacheのSHA256を検証して辞書を復元する。通信失敗時の`.part`は次回online実行で再取得する。cache自体のhashが異なる場合は黙って使わず停止し、エラーに表示したarchiveを削除して再取得する。辞書だけなら`setup-sudachi.ps1`を実行できる。

設定はSudachiに内蔵された文字定義・OOV/rewrite資源を使用し、作業ディレクトリのchar.def等に依存しない最小開発profile。製品用domain profileではない。設定変更も精度へ影響するのでGitで追跡する。

全辞書比較ではcore/fullも20250129を固定し、target配下へ分離する。fullは公式配布ZIPからLEGALを含め保存。[取得と比較](sudachi-dictionary-matrix.md)。

初回はcrates.io、GitHub、files.pythonhosted.org（fullは公式CloudFront配布先）へ依存・辞書取得の通信が必要。準備後の`verify.ps1 -Offline`およびNLP実行ではdownloadしない。GGUFモデルは取得しない。

## テストの区別

| コマンド | 確認するもの |
| --- | --- |
| `cargo check --locked --lib` | default評価facade＋Sudachi。Git/Candle/生成tokenizerを含まない |
| `cargo test --locked --test evaluation_nlp` | owned実辞書、Mode A/B/C、全形態素/span、モデル不要起動、並行評価 |
| `cargo test --locked --no-default-features --test evaluation_contracts` | 別backendを注入するcore-only公開API |
| `cargo test --locked --features legacy --test nlp_resources` | 旧NLP APIと無効GGUF拒否 |
| `cargo test --locked --workspace --all-features` | core・新Sudachi・legacy unit・旧workflowの全試験。通常コンストラクタは本番runtime |
| `scripts/dev/verify.ps1` | 上記とformat、期待する結合・並行性テスト名の登録を確認 |

defaultの`cargo test`では新評価経路だけを実行する。旧本番runtimeは`cargo test -p kaze_nhanh_legacy`、workflow fakeはmock_inferenceを明示する。合成量子化GGUFでCPU forwardとKV cacheを確認するが、P0の学習済みSmolLM2資産は専用ジョブでCPU生成・参照ID・再現性を検証する。日本語判定精度・速度SLOはP3/P4で扱う。[推論資産検証手順](inference_engine.md)に従ってverify-model.ps1へローカルGGUF・tokenizer・参照token IDを渡す。

## Linux CI

Rust stable、C/C++ toolchain、pkg-config、OpenSSL開発headers、PowerShell 7を用意する。Ubuntu runnerの既存環境でsetup-dev.ps1とverify.ps1を使う。ThreadSanitizerと実モデルの計測は通常試験と別ジョブ/別資産で扱う。

## 今回のPCでの確認

2026-10-04: Windows、Rust/Cargo 1.99.0、Visual Studio 2022 CommunityのC++ツールを検出。セットアップで固定辞書のdownload・hash検証・license保存が成功。本番ライブラリのcargo checkが成功。テスト結果は移行タスクに記録する。

TSanジョブはRust公式の[Sanitizer手順](https://doc.rust-lang.org/unstable-book/compiler-flags/sanitizer.html)に沿って、rust-srcと明示target、build-stdを設定。実行結果はCI上で確認する必要があり、WindowsローカルではTSanを実行していない。

性能成果物はCriterionがCRITERION_OUTPUTを明示的に読み、soakはリポジトリ直下のartifacts/soakへ保存する。stdout/stderrのlogと測定JSONを分け、bench失敗は終了codeとして伝播する。これらはモックworkflowの反復であり、常駐SLMのメモリsoakとは別の検証。

## 0.2の構成とモデル不要利用

[移行ガイド](migration-0.2.md)を参照。cargo run --example evaluate -- textはSudachi資産だけで起動し、P2ではprofileの一次screening reportを返す。意味軸は未評価のまま。verify.ps1はdefault/no-default/coreの通常依存を検査し、旧Git/Markdown/Candle等の混入をエラーにする。

## P2 baseline

cargo run --locked --offline --example primary_baseline -- evaluation/primary-baseline.jsonl target/p2-baselineで34件の手作業fixtureを検証し、JSON reportと集計を保存する。CIではWindows/Linuxの成果物を保存。[一次検出の範囲](primary-detection.md)も参照。

P3実験用adapter: optional qwen feature/crateでCPUの自然さpaired-label判定を追加。固定資産セットアップと独立token ID/CPU smokeは[Qwen手順](qwen-judge.md)を参照。実運用品質・意味adapter・CPU SLOは未受入。
