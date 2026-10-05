# kzn-ocr-synth-1k登録（2026-10-06）

ユーザー提供の外部datasetを今後のOCR評価集合として登録した。原本は`C:/Users/Shion/Documents/Projects/kzn-dataset/dataset/kzn-ocr-synth-1k.jsonl`。本文・画像はKazeNhanh repoへ複製せず、`resources/evaluation/kzn-ocr-synth-1k.lock.json`にdataset/manifest/README/ATTRIBUTION/sources.lockのSHA256を固定する。

提供元READMEによると、描画した合成行画像をpure-onnx-ocr 0.2.1 / PP-OCRv6 mediumで実際に認識したデータ。data_kind=synthetic、参照は描画文字列、verifiedは提供元の宣言である。実撮影OCRやASRの品質を保証する集合としては扱わない。画像と転記の独立目視検証は未実施。

## ローカル取り込み

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev/prepare-ocr-synth-1k.ps1 -DatasetRoot C:/Users/Shion/Documents/Projects/kzn-dataset -Offline
```

固定hashを照合し、既存validate_recognition_datasetでmanifest/schema/hash/ID・原文グループ・split監査を実行。その後行のsplitとmanifestも照合する。既存の分割を変更しない。出力はtarget/kzn-ocr-synth-1kのみで、CI実行を拒否する。

| split | 件数 | 今後の用途 |
| --- | ---: | --- |
| train | 500 | 統計資産や推定器の学習 |
| calibration | 200 | estimatorと分離した校正 |
| development | 100 | 特徴・条件検討 |
| test | 200 | 条件固定後の評価 |

各splitについて`*.references.jsonl`は出典・ラベルを含む元行を保持、`*.inputs.jsonl`はid/source/document_id/text/engine/confidence/confidence_scaleのみ許可する。transcription、CER、正誤ラベル、生成条件を推論へ渡さない。将来box情報を使う場合は座標契約を別途追加する。ATTRIBUTION.mdもローカル出力へ保存する。Tatoeba/青空文庫/自作等の出典は参照行に保持し、混合ライセンスを一括CC0としない。

1,000件のsplit監査成功。train/calibration/test/developmentは500/200/200/100。再実行のsplit hash一致、全件原文/confidence保持、推論入力キー制限を確認。confidence=nullの78件を0へ変換しない。監査は宣言されたgroupと同一転記hash等の検査であり、類似テンプレート・同作品などの潜在的依存を自動排除しない。

## 評価へ接続する次の作業

今回の登録ではKazeNhanh推論・閾値探索は実行していない。既存recognition_samplesはnumeric confidenceを要求するため、nullをtyped missingへ変換する対応が先に必要。提供元のconfidence_scaleはCTC emitted-token probabilityのbox文字数重み付き平均との宣言で、転記正解確率ではない。raw値と定義を保持し、confidence ruleのtarget/decoder等を推測で補わない。

trainの参照だけからsmall/core/fullそれぞれの統計assetを作り、developmentで既存rules/統計/候補条件の比較を行う予定。calibration/testの転記を資産学習へ混ぜない。テストを見る前に採用条件を決める。人工画像内の性能と実画像への一般化を区別し、追加の実OCR/ASRが必要になった時点でユーザーに依頼する。画像再生成・OCR/ASR再実行は不要。