# Sudachi small / core / full比較

2026-10-04。R2以降の辞書選択はsmall単独で評価せず、3 editionで同じ評価集合を回す。今回は既存smallと同じ20250129を固定し、Sudachi.rs v0.6.9・Mode C・settings・入力・clean corpusを共通にした。editionとA/B/Cの分割modeは別の要因で、今回はmodeを変えていない。

## 準備と再実行

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev/setup-dev.ps1 -AllDictionaries
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev/evaluate-dictionary-matrix.ps1 -Offline
```

個別取得: `setup-sudachi.ps1 -Edition small` / `-Edition core` / `-Edition full`。全辞書の初回取得後は-Offlineでcache検証・再配置・比較ができる。通常setup/default API/CI smokeのsmallは維持する。core/fullはtarget/sudachi-dictionaries/{edition}/system.dicへ配置し、smallのresources/sudachi/system.dicを上書きしない。

各lockはURL/version/archive SHA256/抽出辞書SHA256を固定。small/coreは公式PyPI wheel、full 20250129は公式PyPI sdist内setup.pyが指定する公式配布ZIPを使う（パッケージコードは実行しない）。full ZIP hashは初回取得bytesから固定したdigestで、上流署名の検証を意味しない。配布元のLICENSE/NOTICE/COPYING/LEGALを保存する。

[small配布](https://pypi.org/project/SudachiDict-small/20250129/) / [core配布](https://pypi.org/project/SudachiDict-core/20250129/) / [full配布](https://pypi.org/project/SudachiDict-full/20250129/)。manifestはresources/sudachi/dictionary.lock.json、dictionary.core.lock.json、dictionary.full.lock.json。

## 統計資産と観察

同じ人工clean8文から辞書ごとに語頻度assetを作る。分割/語彙が変わるので、smallのassetをcore/fullへ流用しない。asset IDには解析identity（辞書/settings/mode）のdigestを含め、assetファイルSHA256と全解析identityを照合する。新しい--dictionary path引数をstatistics_asset / recognition_samplesに追加し、省略時は従来のsmallを使う。

matrixは提供OCR15件のrecognition reportと独立した形態素snapshotを保存し、分割・dictionary_form・POS・読み・OOV・原文span・文字/語統計・decisionを比較する。dictionary_snapshotは解析観察用でgold/referenceを読まない。cold load/単回解析時間はdebug process観察値で、CPU SLOとして扱わない。snapshotは別のoffline解析であり、recognition engine内部の一次/統計の解析1回共有は維持する。

詳細JSONはtarget/dictionary-matrix/summary.jsonとedition別ディレクトリへローカル保存。ユーザー由来の詳細をCIへアップロードしない。matrixはCI環境では実行を拒否し、通常CIではsmall回帰と隔離setup failure試験を行う。独立した実OCR/ASRを追加した際も同じ3辞書比較を行う方針。

## 今回の結果

| 20250129 / Mode C | OCR15件の形態素総数 | OOV総数 | review | undetermined | low_risk |
| --- | ---: | ---: | ---: | ---: | ---: |
| small | 61 | 3 | 0 | 15 | 0 |
| core | 58 | 3 | 0 | 15 | 0 |
| full | 58 | 3 | 0 | 15 | 0 |

smallからcore/fullで3件の分割が変わった。core/fullのOCR15件の分割は一致。OOV数とdecisionは全件同じ、risk=null / SLM呼出0。これは特徴の比較であり、誤認識検出品質の受入ではない。辞書を大きくしても今回の誤認識を検出できたとは言えない。

別の人工hard-clean6例（口語・型番・固有名詞・複合語等）では形態素総数small/core/full=30/29/28、OOV=4/3/3。こちらは形態素snapshotのみで、実ASR測定や認識risk精度ではない。8文の学習corpusとは分離するが、train/calibration/test品質データの代わりにはしない。

現時点で推薦辞書を変更しない。次の実データ比較ではsegment/span検出、誤警報/保留、自然な誤認識、辞書別の統計/fusion、cold/warm latency・RSS・資産容量も比較する。未知語を辞書が認識することと、原資料を忠実に転記したことは別。辞書別に校正・profileを検証する。
