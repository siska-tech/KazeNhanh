"""Generate P0 references with official HF tokenizer, independently of Rust.
Run after setup-model.ps1 using transformers==4.46.3, tokenizers==0.20.3.
No network access: official assets are loaded from the verified local directory.
"""
import argparse
import json
from pathlib import Path
import transformers
import tokenizers
from transformers import AutoTokenizer

parser = argparse.ArgumentParser()
parser.add_argument("asset_dir", type=Path)
parser.add_argument("output", type=Path)
args = parser.parse_args()
if transformers.__version__ != "4.46.3" or tokenizers.__version__ != "0.20.3":
    raise RuntimeError("Use transformers==4.46.3 and tokenizers==0.20.3")
tokenizer = AutoTokenizer.from_pretrained(args.asset_dir, local_files_only=True)
texts = ["東京都で自然な日本語を解析します。", "OCR: 請求額は1,234円です。 ASR: えー、明日。", "自然🙂\nＡＢＣ cafe\u0301", "<|im_start|>user\nHello!<|im_end|>\n<|im_start|>assistant\n"]
cases = []
requests = ["Say hello in one short sentence.", "What is 2 + 2? Answer briefly.", "Name one color. Answer in one word.", "Say goodbye in one short sentence."]
for text, request in zip(texts, requests):
    prompt = tokenizer.apply_chat_template([
        {"role": "system", "content": "You are a helpful assistant. Answer briefly."},
        {"role": "user", "content": request},
    ], tokenize=False, add_generation_prompt=True).strip()
    cases.append({"text": text, "expected_ids": tokenizer.encode(text, add_special_tokens=True),
                  "prompt": prompt, "expected_prompt_ids": tokenizer.encode(prompt, add_special_tokens=True)})
args.output.write_text(json.dumps(cases, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print(f"Generated {len(cases)} references: transformers={transformers.__version__}, tokenizers={tokenizers.__version__}")
