//! Offline corpus builder. The corpus must contain clean training texts, never evaluation labels.
use kaze_nhanh::*;
use serde::Deserialize;
use sha2::{Digest, Sha256};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CleanSegment {
    id: String,
    document_id: String,
    text: String,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args: Vec<_> = std::env::args().skip(1).collect();
    let dictionary_path = if args.len() >= 2 && args[args.len() - 2] == "--dictionary" {
        let path = std::path::PathBuf::from(args.pop().expect("path argument"));
        args.pop();
        Some(path)
    } else {
        None
    };
    if args.len() != 5 {
        return Err(
            "Usage: statistics_asset clean.jsonl corpus-id domain license output.json [--dictionary path]".into(),
        );
    }
    if std::fs::metadata(&args[0])?.len() > 8 * 1024 * 1024 {
        return Err("corpus exceeds 8 MiB".into());
    }
    let bytes = std::fs::read(&args[0])?;
    let data = std::str::from_utf8(&bytes)?;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let analyzer = SudachiAnalyzer::new(SudachiConfig::from_paths(
        dictionary_path.unwrap_or_else(|| root.join("resources/sudachi/system.dic")),
        root.join("resources/sudachi/sudachi.json"),
        SudachiMode::C,
    )?)?;
    let mut ids = std::collections::HashSet::new();
    let mut documents = Vec::new();
    for line in data.lines().filter(|line| !line.trim().is_empty()) {
        if documents.len() >= 4096 {
            return Err("corpus exceeds 4096 segments".into());
        }
        let sample: CleanSegment = serde_json::from_str(line)?;
        if sample.id.trim().is_empty()
            || sample.document_id.trim().is_empty()
            || !ids.insert((sample.document_id, sample.id))
        {
            return Err("empty/duplicate corpus identity".into());
        }
        if sample.text.len() > 65_536 {
            return Err("segment exceeds 65536 bytes".into());
        }
        let analysis = analyzer.analyze(&sample.text)?;
        documents.push((sample.text, analysis));
    }
    let identities = documents
        .first()
        .ok_or("empty clean corpus")?
        .1
        .provenance
        .clone();
    let analyzer_key = format!("{:x}", Sha256::digest(serde_json::to_vec(&identities)?));
    let artifact = StatisticsArtifact::fit(
        StatisticsMetadata {
            id: format!(
                "{}.{}.statistics.v1.{}",
                args[1],
                args[2],
                &analyzer_key[..16]
            ),
            domain: args[2].clone(),
            corpus_id: args[1].clone(),
            corpus_sha256: format!("{:x}", Sha256::digest(&bytes)),
            license: args[3].clone(),
            analyzer: identities,
        },
        &documents,
    )?;
    let output = serde_json::to_vec_pretty(&artifact)?;
    let path = std::path::Path::new(&args[4]);
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, &output)?;
    println!(
        "{}",
        serde_json::json!({"asset_id":artifact.metadata.id,"asset_sha256":format!("{:x}",Sha256::digest(&output)),
        "corpus_sha256":artifact.metadata.corpus_sha256,"segment_count":artifact.document_count,
        "quality_accepted":false})
    );
    Ok(())
}
