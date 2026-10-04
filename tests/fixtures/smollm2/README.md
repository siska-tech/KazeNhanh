# SmolLM2 P0 compatibility fixture

- Official checkpoint: https://huggingface.co/HuggingFaceTB/SmolLM2-135M-Instruct/tree/12fd25f77366fa6b3b4b768ec3050bf629380bac
- GGUF conversion: https://huggingface.co/bartowski/SmolLM2-135M-Instruct-GGUF/tree/09816acd5d99df7be770d85ea30822623dab342c
- Both model cards declare Apache-2.0. This is an English model for CPU compatibility smoke, not the selected Japanese evaluation judge.
- Exact download URLs and SHA256: `resources/models/smollm2.lock.json`.
- References generated with CPython 3.12.14, transformers 4.46.3, tokenizers 0.20.3 and Jinja2 3.1.6 using the official tokenizer and chat template. Rust uses tokenizers 0.19.1.
- `add_special_tokens=True`; Japanese, mixed OCR/ASR text, emoji, fullwidth and combining characters, and explicit special tokens. `expected_prompt_ids` also checks the official chat template.
- Regenerate using `scripts/dev/generate-model-references.py`; requires only tokenizer dependencies, no PyTorch/model weights in Python. Regeneration must exactly reproduce the checked-in JSON.
- Each of four different prompts runs twice through the same production Candle CPU engine. The runner checks nonempty generation and repeated output equality after resetting KV state. Output quality and speed SLOs belong to P3/P4.
