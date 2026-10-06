//! Offline, metadata-only split audit. References never enter recognition inference.
use kaze_nhanh::RecognitionSource;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Split {
    Train,
    Calibration,
    Test,
    Development,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum DataKind {
    Measured,
    Synthetic,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: String,
    dataset_id: String,
    dataset_sha256: String,
    provenance: String,
    revision: String,
    license: String,
    data_kind: DataKind,
    items: Vec<Item>,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Item {
    id: String,
    source: RecognitionSource,
    document_id: String,
    /// Same original utterance/text across corruptions, N-best and views.
    origin_id: String,
    speaker_ids: Vec<String>,
    session_ids: Vec<String>,
    split: Split,
}
#[derive(Deserialize)]
struct Sample {
    id: String,
    source: RecognitionSource,
    document_id: String,
    text: String,
    transcription: String,
    transcription_status: String,
    comparison_policy: String,
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn identity(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 4096 && value.trim() == value
}
fn insert_group(
    groups: &mut BTreeMap<(String, String), Split>,
    kind: &str,
    id: &str,
    split: Split,
) -> Result<()> {
    if !identity(id) {
        return Err(format!("invalid {kind} identity").into());
    }
    if groups
        .insert((kind.into(), id.into()), split)
        .is_some_and(|previous| previous != split)
    {
        return Err(format!("cross-split overlap in {kind}").into());
    }
    Ok(())
}
fn audit(manifest: &Manifest, bytes: &[u8]) -> Result<Value> {
    if manifest.schema_version != "kzn.recognition.dataset.v1"
        || [
            &manifest.dataset_id,
            &manifest.provenance,
            &manifest.revision,
            &manifest.license,
        ]
        .iter()
        .any(|s| !identity(s))
        || manifest.dataset_sha256 != digest(bytes)
        || manifest.items.is_empty()
        || manifest.items.len() > 4096
    {
        return Err("invalid manifest identity/schema/hash/size".into());
    }
    let mut samples = BTreeMap::new();
    for line in std::str::from_utf8(bytes)?
        .lines()
        .filter(|l| !l.trim().is_empty())
    {
        let sample: Sample = serde_json::from_str(line)?;
        if samples.len() >= 4096
            || !identity(&sample.id)
            || samples.insert(sample.id.clone(), sample).is_some()
        {
            return Err("invalid/duplicate/too many sample IDs".into());
        }
    }
    let mut groups = BTreeMap::new();
    let mut counts = BTreeMap::<Split, usize>::new();
    let mut source_counts = BTreeMap::<String, usize>::new();
    let mut confirmed_count = 0;
    for item in &manifest.items {
        let sample = samples
            .remove(&item.id)
            .ok_or("missing/duplicate manifest item")?;
        if sample.source != item.source
            || sample.document_id != item.document_id
            || !matches!(
                sample.comparison_policy.as_str(),
                "raw.v1" | "ignore_leading_bullet.v1"
            )
            || sample.text.len() > 65_536
            || sample.transcription.len() > 65_536
        {
            return Err("source/document/policy/length mismatch".into());
        }
        match sample.transcription_status.as_str() {
            "verified" => confirmed_count += 1,
            "unconfirmed" if item.split == Split::Development => {}
            _ => {
                return Err(
                    "unconfirmed references belong only in development; unknown status rejected"
                        .into(),
                )
            }
        }
        if item.speaker_ids.len() > 64
            || item.session_ids.len() > 64
            || (item.source == RecognitionSource::Asr
                && (item.speaker_ids.is_empty() || item.session_ids.is_empty()))
        {
            return Err("ASR requires speaker/session grouping; group limit is 64".into());
        }
        insert_group(&mut groups, "document", &item.document_id, item.split)?;
        insert_group(&mut groups, "origin", &item.origin_id, item.split)?;
        // Conservative exact-content guard even if origin IDs were accidentally renamed.
        // Empty references do not establish shared content identity (e.g. silence).
        if !sample.transcription.is_empty() {
            insert_group(
                &mut groups,
                "reference_sha256",
                &digest(sample.transcription.as_bytes()),
                item.split,
            )?;
        }
        for (kind, ids) in [
            ("speaker", &item.speaker_ids),
            ("session", &item.session_ids),
        ] {
            let mut unique = BTreeSet::new();
            for id in ids {
                if !unique.insert(id) {
                    return Err("duplicate group identity in item".into());
                }
                insert_group(&mut groups, kind, id, item.split)?;
            }
        }
        *counts.entry(item.split).or_default() += 1;
        *source_counts
            .entry(
                match item.source {
                    RecognitionSource::Ocr => "ocr",
                    RecognitionSource::Asr => "asr",
                }
                .into(),
            )
            .or_default() += 1;
    }
    if !samples.is_empty() {
        return Err("unassigned dataset rows".into());
    }
    Ok(
        json!({"schema_version":"kzn.recognition.dataset_audit.v1", "dataset_id":manifest.dataset_id,
        "dataset_sha256":manifest.dataset_sha256, "data_kind":manifest.data_kind,
        "case_count":manifest.items.len(), "confirmed_count":confirmed_count,
        "split_counts":counts, "source_counts":source_counts,
        "declared_groups_disjoint":true, "quality_accepted":false,
        "limitations":["metadata declarations and verification status are not independently authenticated",
        "exact reference hash check does not detect paraphrases or unreported shared sources",
        "no train/calibration/test size, class balance or domain adequacy acceptance"]}),
    )
}
fn read(path: &std::ffi::OsStr) -> Result<Vec<u8>> {
    if std::fs::metadata(path)?.len() > 16 * 1024 * 1024 {
        return Err("input exceeds 16 MiB".into());
    }
    Ok(std::fs::read(path)?)
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 {
        return Err(
            "Usage: validate_recognition_dataset manifest.json dataset.jsonl audit.json".into(),
        );
    }
    let manifest_bytes = read(&args[0])?;
    let dataset_bytes = read(&args[1])?;
    let manifest: Manifest = serde_json::from_slice(&manifest_bytes)?;
    let mut result = audit(&manifest, &dataset_bytes)?;
    result["manifest_sha256"] = json!(digest(&manifest_bytes));
    let output = std::path::Path::new(&args[2]);
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(output, serde_json::to_string_pretty(&result)?)?;
    println!("{result}");
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Manifest, Vec<u8>) {
        let bytes = include_bytes!("../tests/fixtures/dataset/synthetic.jsonl").to_vec();
        let manifest = serde_json::from_str(include_str!(
            "../tests/fixtures/dataset/synthetic.manifest.json"
        ))
        .unwrap();
        (manifest, bytes)
    }
    #[test]
    fn disjoint_sources_and_splits_pass_with_pinned_content() {
        let (m, b) = fixture();
        let v = audit(&m, &b).unwrap();
        assert_eq!(v["case_count"], 4);
        assert_eq!(v["confirmed_count"], 3);
        assert_eq!(v["source_counts"]["asr"], 2);
        assert_eq!(v["quality_accepted"], false);
    }
    #[test]
    fn every_group_axis_blocks_leakage_including_candidate_views() {
        let (m, b) = fixture();
        for axis in ["document", "origin", "speaker", "session"] {
            let mut bad = m.clone();
            match axis {
                "document" => bad.items[1].document_id = bad.items[0].document_id.clone(),
                "origin" => bad.items[1].origin_id = bad.items[0].origin_id.clone(),
                "speaker" => bad.items[1].speaker_ids = bad.items[0].speaker_ids.clone(),
                _ => bad.items[1].session_ids = bad.items[0].session_ids.clone(),
            }
            assert!(audit(&bad, &b).is_err(), "{axis}");
        }
        for kind in ["document", "origin", "speaker", "session"] {
            let mut groups = BTreeMap::new();
            insert_group(&mut groups, kind, "shared", Split::Train).unwrap();
            insert_group(&mut groups, kind, "shared", Split::Train).unwrap();
            assert!(insert_group(&mut groups, kind, "shared", Split::Test)
                .unwrap_err()
                .to_string()
                .contains("cross-split overlap"));
        }
    }
    #[test]
    fn hashes_missing_groups_and_unconfirmed_test_are_rejected() {
        let (m, b) = fixture();
        let mut bad = m.clone();
        bad.dataset_sha256 = "0".repeat(64);
        assert!(audit(&bad, &b).is_err());
        let mut bad = m.clone();
        bad.items[0].speaker_ids.clear();
        assert!(audit(&bad, &b).is_err());
        let mut bad = m.clone();
        bad.items[0].session_ids.clear();
        assert!(audit(&bad, &b).is_err());
        let mut bad = m.clone();
        bad.items[3].split = Split::Test;
        assert!(audit(&bad, &b).is_err());
        let mut bad = m.clone();
        bad.items.pop();
        assert!(audit(&bad, &b).is_err());
        let mut bad = m.clone();
        bad.items.push(bad.items[0].clone());
        assert!(audit(&bad, &b).is_err());
    }
    #[test]
    fn renamed_origins_do_not_hide_exact_reference_overlap() {
        let (mut m, b) = fixture();
        let mut rows: Vec<Value> = std::str::from_utf8(&b)
            .unwrap()
            .lines()
            .map(|s| serde_json::from_str(s).unwrap())
            .collect();
        rows[1]["transcription"] = rows[0]["transcription"].clone();
        let bytes = rows
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n")
            .into_bytes();
        m.dataset_sha256 = digest(&bytes);
        assert!(audit(&m, &bytes)
            .unwrap_err()
            .to_string()
            .contains("reference_sha256"));
    }
}
