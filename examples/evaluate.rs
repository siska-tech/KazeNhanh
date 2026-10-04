//! P1 report contract demo: morphology is available, detection is not yet configured.
use kaze_nhanh::{japanese_engine, EvaluationConfig, SudachiConfig, SudachiMode, TextInput};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let text = std::env::args()
        .nth(1)
        .ok_or("Usage: cargo run --example evaluate -- text")?;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let assets = SudachiConfig::from_paths(
        root.join("resources/sudachi/system.dic"),
        root.join("resources/sudachi/sudachi.json"),
        SudachiMode::C,
    )?;
    let engine = japanese_engine(assets, EvaluationConfig::default())?;
    println!(
        "{}",
        serde_json::to_string_pretty(&engine.evaluate(TextInput::new(&text))?)?
    );
    Ok(())
}
