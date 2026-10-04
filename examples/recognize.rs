//! R0 model-free recognition evidence CLI. Unflagged inputs remain undetermined.
use kaze_nhanh::*;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 4 {
        return Err("Usage: recognize text ocr|asr document-id segment-id".into());
    }
    let source: RecognitionSource =
        serde_json::from_value(serde_json::Value::String(args[1].clone()))?;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let assets = SudachiConfig::from_paths(
        root.join("resources/sudachi/system.dic"),
        root.join("resources/sudachi/sudachi.json"),
        SudachiMode::C,
    )?;
    let engine = japanese_recognition_engine(assets, RecognitionConfig::default())?;
    let report =
        engine.evaluate_recognition(RecognitionInput::new(&args[0], source, &args[2], &args[3]))?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
