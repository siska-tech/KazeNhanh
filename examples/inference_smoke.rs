//! Opt-in smoke runner for locally supplied trained LLaMA GGUF resources.
//! Usage: cargo run --example inference_smoke -- model.gguf tokenizer.json cases.json
//! No model download or quality evaluation is performed.
use std::error::Error;
use std::path::PathBuf;
use std::time::Instant;

use kaze_nhanh::{EngineConfig, KazeNhanhEngine};
use serde::{Deserialize, Serialize};
use tokenizers::Tokenizer;

#[derive(Deserialize)]
struct ReferenceCase {
    text: String,
    expected_ids: Vec<u32>,
    #[serde(default)]
    expected_prompt_ids: Vec<u32>,
    // A model-specific chat template can be supplied by the reference author.
    prompt: String,
}

#[derive(Serialize)]
struct SmokeResult {
    case_index: usize,
    reference_ids_match: bool,
    first_inference_ms: u128,
    repeat_inference_ms: u128,
    repeat_output_matches: bool,
    output: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    if arguments.len() != 3 {
        return Err("Usage: inference_smoke model.gguf tokenizer.json reference-cases.json".into());
    }
    let model = std::fs::read(&arguments[0])?;
    let tokenizer_bytes = std::fs::read(&arguments[1])?;
    let references: Vec<ReferenceCase> = serde_json::from_slice(&std::fs::read(&arguments[2])?)?;
    if references.is_empty() {
        return Err("Reference cases must not be empty".into());
    }
    let tokenizer = Tokenizer::from_bytes(&tokenizer_bytes).map_err(|err| err.to_string())?;
    for (index, case) in references.iter().enumerate() {
        if case.text.trim().is_empty()
            || case.prompt.trim().is_empty()
            || case.expected_ids.is_empty()
        {
            return Err(format!("Empty reference case at index {index}").into());
        }
        let actual = tokenizer
            .encode(case.text.as_str(), true)
            .map_err(|err| err.to_string())?;
        if actual.get_ids() != case.expected_ids {
            return Err(format!(
                "Reference token IDs mismatch at case {index}: actual={:?}",
                actual.get_ids()
            )
            .into());
        }
    }
    for (index, case) in references.iter().enumerate() {
        if !case.expected_prompt_ids.is_empty() {
            let actual = tokenizer
                .encode(case.prompt.as_str(), true)
                .map_err(|err| err.to_string())?;
            if actual.get_ids() != case.expected_prompt_ids {
                return Err(format!("Chat template token IDs mismatch at case {index}").into());
            }
        }
    }
    // The legacy EngineConfig owns static resources. These one-shot CLI buffers
    // intentionally live until process exit; P1 will introduce owned resources.
    let dictionary = std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/sudachi/system.dic"),
    )?;
    let config = EngineConfig::new(
        Box::leak(model.into_boxed_slice()),
        Box::leak(dictionary.into_boxed_slice()),
        include_bytes!("../resources/sudachi/sudachi.json"),
    );
    let engine = KazeNhanhEngine::new_with_tokenizer(config, &tokenizer_bytes)?;
    for (index, case) in references.iter().enumerate() {
        let start = Instant::now();
        let output = engine.synthesize_summary(vec![case.prompt.clone()])?;
        let first_ms = start.elapsed().as_millis();
        if output.is_empty() {
            return Err(format!("Empty generated output at case {index}").into());
        }
        let start = Instant::now();
        let repeated = engine.synthesize_summary(vec![case.prompt.clone()])?;
        if repeated != output {
            return Err(format!("Repeated output differs at case {index}").into());
        }
        println!(
            "{}",
            serde_json::to_string(&SmokeResult {
                case_index: index,
                reference_ids_match: true,
                first_inference_ms: first_ms,
                repeat_inference_ms: start.elapsed().as_millis(),
                repeat_output_matches: true,
                output,
            })?
        );
    }
    Ok(())
}
