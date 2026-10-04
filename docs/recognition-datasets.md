# 評価データの取得とsplit監査

2026-10-05。R2の独立実OCR/ASR集合は未整備。`validate_recognition_dataset`は、取り込むデータの同一性と宣言した分割単位を検証するオフライン補助ツール。推論や学習を実行せず、品質受入も行わない。

## 分割契約

manifest schemaは`kzn.recognition.dataset.v1`。dataset_id、JSONL全bytesのSHA256、provenance、revision、license、data_kind（measured/synthetic）とitemsを必須とする。manifestの未知field/enumを拒否する。実測か、確認済みか、利用条件が適切かという宣言の真偽は人が確認する。

各itemはid/source/document_id/origin_id/speaker_ids/session_ids/splitを持つ。splitはtrain/calibration/test/development。同一document・origin・speaker・sessionは全て同じsplitでなければならない。複数話者の録音では全speaker IDを列挙し、同じ人物を別IDで宣言しない。ASRでは話者とsessionの空配列を拒否する。OCRでは話者/sessionを空にできるが、原画像documentと原文originは必要。候補・人工破損・複数viewには共通origin_idを付ける。候補内容をgoldから生成しない。

同じ非空転記のSHA256が異なるsplitへ入った場合も拒否する。origin IDの付け間違いへの保守的な防御であり、独立に発話された同じ短文も止まる。その場合は同じsplitにまとめる。空転記だけでは同一原文と判定せず、document/origin等の宣言で分離を確認する。表記違い・類似文・隠れた同一話者は検出できない。

JSONLはid/source/document_id/text/transcription/transcription_status/comparison_policyを明示する。statusはverified/unconfirmed。未確認はdevelopmentだけに許し、train/calibration/testへ混入させない。比較規約はraw.v1/ignore_leading_bullet.v1（[品質集計](recognition-quality.md)）。付加metadataは保持可能だが監査では読み飛ばす。全行とmanifestを一対一照合し、不足/重複/余分なID、source/document不一致を拒否する。

最大4,096件、各入力ファイル16 MiB、各text/転記65,536 bytes、話者/sessionは各64 IDs。IDは空白だけ・前後空白・4,096 bytes超過を拒否する。パスをmanifestから開いたり、URLから自動取得したりしない。

```powershell
cargo test --locked --offline --no-default-features --example validate_recognition_dataset
cargo run --locked --offline --no-default-features --example validate_recognition_dataset -- tests/fixtures/dataset/synthetic.manifest.json tests/fixtures/dataset/synthetic.jsonl target/dataset-contract-audit.json
```

出力はsplit/source別件数、確認済み件数、両入力hash、declared_groups_disjoint。`quality_accepted=false`を維持する。群の独立性・testを見ていないこと・サイズ/クラスバランス・domain代表性の保証にはしない。既存quality runnerは独立した観察器のままで、監査未実施データをheld-out testと称してはいけない。人工4件fixtureはAPI契約だけを検証し、実ASR/OCRの性能評価には使わない。

## GitHubサンプルの取得結果

[ouktlab/asr-ja_evalkit](https://github.com/ouktlab/asr-ja_evalkit/tree/837f8ccc1f8b97d492a94f4c14b490a21df98920)のsample/egs_hyplist.txtとegs_reflist.txtは1対。READMEの表記揺れ説明と対応する。認識器・音声・録音条件を確認できないため、独立した実ASR品質データに採用しない。READMEには研究/教育向けの記載がある。リポジトリのLICENSEはApache-2.0、著作権表記はKomatani Laboratory (Ryu Takeda)。実データの利用条件とコードのライセンスを混同しない。

固定commit、4ファイル（サンプル対/README/LICENSE）のbytes/SHA256を[lock](../resources/evaluation/asr-ja-evalkit.lock.json)に保存。原ファイルを変更せずtarget/asr-ja-evalkit-sampleへ取得し、repoには再配布しない。認識器実行、音声ダウンロード、第三者スクリプト実行はない。上流の参照依存正規化も適用しない。

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev/setup-asr-sample.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev/setup-asr-sample.ps1 -Offline
```

キャッシュ欠落はOfflineで失敗、既存キャッシュ/取得bytesの不一致は停止。取得した説明例はmeasured/verifiedへ昇格させない。今回の公開サンプルは理解のための取得で、学習・校正・受入testに含めない。

次は認識器と取得条件を追跡できる実認識出力を確保し、原資料確認・許諾・group IDを整備する。既存の提供OCR15件はdevelopment観察用として維持する。small/core/fullの同版比較方針は変更しない。