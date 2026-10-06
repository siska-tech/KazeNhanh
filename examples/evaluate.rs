//! Primary screening JSON CLI. No model loading or correction generation.
use kaze_nhanh::{japanese_engine, DomainProfile, SudachiConfig, SudachiMode, TextInput};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() || args.len() > 4 {
        return Err("Usage: evaluate text [profile-id] [source-kind] [reference]".into());
    }
    let profile =
        DomainProfile::builtin(args.get(1).map(String::as_str).unwrap_or("ja.primary.v1"))?;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let assets = SudachiConfig::from_paths(
        root.join("resources/sudachi/system.dic"),
        root.join("resources/sudachi/sudachi.json"),
        SudachiMode::C,
    )?;
    let engine = japanese_engine(assets, profile.evaluation_config())?;
    let mut input = TextInput::new(&args[0]);
    if let Some(source) = args.get(2) {
        input.source = serde_json::from_value(serde_json::Value::String(source.clone()))?;
    }
    input.reference = args.get(3).map(String::as_str);
    println!(
        "{}",
        serde_json::to_string_pretty(&engine.evaluate(input)?)?
    );
    Ok(())
}
