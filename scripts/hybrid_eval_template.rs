//! Hybrid summarizer evaluation template.
//!
//! This executable is a starting point for running batch evaluations against
//! the `HybridSummarizer`. Replace the placeholder asset loaders with real
//! GGUF / Sudachi resources and wire it into your dataset iterator. The
//! resulting JSON lines can be consumed by external ROUGE tooling.

use std::error::Error;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use kaze_nhanh::{EngineConfig, KazeNhanhEngine};
use serde::Serialize;

#[derive(Debug, Serialize)]
struct SummaryRecord {
    source_id: String,
    extracted_sentences: Vec<String>,
    synthesized_summary: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    // TODO: swap these placeholders for real assets (e.g. include_bytes!).
    let model_bytes: &'static [u8] = b"mock-model";
    let dictionary_bytes: &'static [u8] = b"mock-dictionary";
    let settings_bytes: &'static [u8] = br#"{"dict":"system"}"#;

    // Dataset is expected to be a text file with `id\tcontent` per line.
    let input_path = PathBuf::from("./data/sample_documents.tsv");
    let output_path = PathBuf::from("./artifacts/hybrid_summaries.jsonl");

    std::fs::create_dir_all(output_path.parent().unwrap())?;

    let config = EngineConfig::new(model_bytes, dictionary_bytes, settings_bytes);
    let engine = KazeNhanhEngine::new(config)?;

    let reader = BufReader::new(File::open(&input_path)?);
    let mut writer = File::create(&output_path)?;

    for line in reader.lines() {
        let raw = line?;
        if raw.trim().is_empty() {
            continue;
        }

        let (identifier, content) = raw
            .split_once('\t')
            .ok_or_else(|| format!("invalid line format: {raw}"))?;

        let (extracted, summary) = engine.summarize_with_details(content)?;
        let record = SummaryRecord {
            source_id: identifier.to_string(),
            extracted_sentences: extracted,
            synthesized_summary: summary,
        };

        serde_json::to_writer(&mut writer, &record)?;
        writer.write_all(b"\n")?;
    }

    println!(
        "Evaluation artifacts written to {}",
        output_path.display()
    );

    Ok(())
}

