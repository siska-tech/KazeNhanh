# Qwen independent tokenizer fixture

Official Qwen2.5-0.5B-Instruct tokenizer revision 7ae557604adf67be50417f59c2c2f167def9a775. Generated offline by Transformers 4.46.3/tokenizers 0.20.3 using the official chat template, not the Rust prompt formatter. Full text IDs and prompt IDs are checked by Rust tokenizers 0.19.1 before CPU smoke.

Use the pinned Python environment described in docs/inference_engine.md, then run:

```powershell
python scripts/dev/generate-judge-references.py target/qwen-judge target/qwen-reference-regenerated.json
```

Compare the generated file with reference-cases.json. The malformed text examples test tokenizer/model execution, not accepted Japanese detection quality. Prompt identity kzn.qwen.naturalness-pair.v1, paired scores uncalibrated. See docs/qwen-judge.md.