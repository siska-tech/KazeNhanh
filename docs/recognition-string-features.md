# 文字種観測（R2、2026-10-05）

RecognitionReportのoptional `string_features`は原文Unicode scalarの数・文字種別件数・隣接文字の文字種が変わる位置を返す。methodは`raw_scalar_ranges.v1`。辞書、学習コーパス、モデルは不要。OCR/ASRで同じ処理を使い、decision/riskには影響させない。混在する型番や単位を異常と断定しない。

分類は固定範囲であり、Unicode Script/General Categoryの完全な分類ではない。

| class | 範囲 |
| --- | --- |
| ascii_letter | A–Z / a–z |
| ascii_digit | 0–9 |
| ascii_whitespace | U+0009–000D / U+0020 |
| hiragana_block | U+3040–309F |
| katakana_block | U+30A0–30FF（長音符・中点等も含む） |
| han_basic_block | U+4E00–9FFF |
| other | 上記以外。全角英数字、半角カナ、拡張漢字、絵文字等も含む |

正規化せず、結合文字も独立scalarとして数える。分類名は文字の言語や正しさを示さない。辞書・Unicodeデータベース更新に依存しない範囲契約とし、将来分類を変更する場合はmethodを変更する。

`counts`は非ゼロのクラスのみ。空文はscalar_count=0、counts={}。`transition_count`は文字種が変わった回数で、同種の隣接は含まない。`transitions`には原文順の先頭64件だけを保持し、残数を`omitted_transitions`に記録する。spanは隣接2文字を囲むUTF-8 byte位置。空白を飛ばした接続や文書間接続は作らない。線形時間、保持するクラス数・位置数は有界。

`RecognitionReport::validate`は原文から再計算してmethod・件数・span・省略数を照合する。エンジンは毎回Someを返すが、旧JSONの欠落/nullはNoneとして読み込み、未観測とする。旧readerは追加フィールドを拒否し得るため更新が必要。公開Rust型をliteralで構築する側は`string_features: None`を追加する。namespaceは`kzn.recognition.v2`を維持し、既存フィールドの意味は変更しない。

`scripts/dev/evaluate-dictionary-matrix.ps1 -Offline -WithPos`は各辞書のsummaryへ文字種観測も保存する。辞書間で同じ原文の観測が一致することを比較できる。詳細はtarget内のローカル出力のみ。これは特徴抽出の検証であり、認識誤り検出の品質受入ではない。