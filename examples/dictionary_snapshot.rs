//! Local morphology observation; reference/gold fields are never loaded into the analyzer.
use kaze_nhanh::*;
use sha2::{Digest, Sha256};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 {
        return Err("Usage: dictionary_snapshot dictionary.dic input.jsonl output.json".into());
    }
    if std::fs::metadata(&args[1])?.len() > 8 * 1024 * 1024 {
        return Err("input exceeds 8 MiB".into());
    }
    let bytes = std::fs::read(&args[1])?;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let began = std::time::Instant::now();
    let analyzer = SudachiAnalyzer::new(SudachiConfig::from_paths(
        &args[0],
        root.join("resources/sudachi/sudachi.json"),
        SudachiMode::C,
    )?)?;
    let load_ms = began.elapsed().as_secs_f64() * 1000.0;
    let mut ids = std::collections::HashSet::new();
    let mut rows = Vec::new();
    for line in std::str::from_utf8(&bytes)?
        .lines()
        .filter(|s| !s.trim().is_empty())
    {
        if rows.len() >= 4096 {
            return Err("input exceeds 4096 segments".into());
        }
        let row: serde_json::Value = serde_json::from_str(line)?;
        let id = row["id"].as_str().ok_or("id required")?;
        let text = row["text"].as_str().ok_or("text required")?;
        if text.len() > 65_536 || !ids.insert(id.to_owned()) {
            return Err("oversized or duplicate segment".into());
        }
        let began = std::time::Instant::now();
        let analysis = analyzer.analyze(text)?;
        rows.push(serde_json::json!({"id":id,"original_text":text,
            "analysis_ms":began.elapsed().as_secs_f64()*1000.0,
            "morphemes":analysis.morphemes,"provenance":analysis.provenance}));
    }
    if rows.is_empty() {
        return Err("empty input".into());
    }
    let output = serde_json::json!({"schema_version":"kzn.dictionary.snapshot.v1","mode":"C",
        "input_sha256":format!("{:x}",Sha256::digest(bytes)),"dictionary_bytes":std::fs::metadata(&args[0])?.len(),
        "load_ms":load_ms,"timing_semantics":"single_debug_process_observation_not_slo",
        "gold_used_for_inference":false,"reports":rows});
    let path = std::path::PathBuf::from(&args[2]);
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_vec_pretty(&output)?)?;
    Ok(())
}
