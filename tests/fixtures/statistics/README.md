# 統計契約fixture

`clean-contract.jsonl`は本実装のために作成した8件の人工clean文（CC0-1.0）。出所はこのリポジトリの契約試験であり、実OCR/ASRの代表コーパスではない。認識出力・gold対ではない。user OCR15件の転記やラベルは学習に使用していない。

対象domainは`contract_fixture`。このdomainを明示した実行は機能観察に限り、ユーザーOCRの適用domainとして受入済みとはしない。学習/校正/test分割・品質受入は別途必要。生成するassetはtarget配下へ置き、辞書/settings/mode/corpus hashを固定して再現性を検証する。

追加のhard-clean.jsonlは6件の人工clean観察例（CC0-1.0）。口語・型番・固有名詞・複合語と原文表記を含む。clean-contract8文とは分け、辞書editionの形態素/OOV比較に使う。実OCR/ASRの認識対・品質testではない。
