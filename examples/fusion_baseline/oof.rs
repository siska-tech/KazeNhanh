//! Deterministic group folds and corpus membership proofs for this offline experiment.
use super::*;
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Plan {
    schema_version: String,
    reference_sha256: String,
    manifest_sha256: String,
    origins: BTreeMap<String, String>,
    pub folds: Vec<Fold>,
    full_corpus_sha256: String,
    full_count: usize,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Fold {
    pub index: usize,
    pub heldout_ids: Vec<String>,
    pub fit_ids: Vec<String>,
    pub corpus_sha256: String,
}
#[derive(Serialize)]
struct Clean<'a> {
    id: &'a str,
    document_id: &'a str,
    text: &'a str,
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn parse(refs: &[u8]) -> Result<Vec<(Reference, Value)>> {
    let mut rows = Vec::new();
    let mut ids = BTreeSet::new();
    for line in std::str::from_utf8(refs)?
        .lines()
        .filter(|l| !l.trim().is_empty())
    {
        let r: Reference = serde_json::from_str(line)?;
        if r.split != "train"
            || r.transcription_status != "verified"
            || r.comparison_policy != "raw.v1"
            || !ids.insert(r.id.clone())
            || rows.len() >= 4096
        {
            return Err("invalid OOF training references".into());
        }
        rows.push((r, serde_json::from_str(line)?));
    }
    if rows.len() < 5 {
        return Err("OOF requires at least five rows".into());
    }
    Ok(rows)
}
fn corpus(rows: &[(Reference, Value)], ids: &BTreeSet<&str>) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    for (r, _) in rows.iter().filter(|(r, _)| ids.contains(r.id.as_str())) {
        serde_json::to_writer(
            &mut bytes,
            &Clean {
                id: &r.id,
                document_id: &r.document_id,
                text: &r.transcription,
            },
        )?;
        bytes.push(b'\n');
    }
    Ok(bytes)
}
fn partitions(
    rows: &[(Reference, Value)],
    origins: &BTreeMap<String, String>,
) -> Result<Vec<Vec<usize>>> {
    if origins.len() != rows.len()
        || rows
            .iter()
            .any(|(r, _)| origins.get(&r.id).is_none_or(|v| v.trim().is_empty()))
    {
        return Err("OOF origin mapping incomplete".into());
    }
    let mut parent: Vec<usize> = (0..rows.len()).collect();
    fn root(p: &[usize], mut i: usize) -> usize {
        while p[i] != i {
            i = p[i];
        }
        i
    }
    let mut seen = BTreeMap::new();
    for (i, (r, _)) in rows.iter().enumerate() {
        let mut keys = vec![
            format!("doc:{}", r.document_id),
            format!("origin:{}", origins[&r.id]),
        ];
        if !r.transcription.is_empty() {
            keys.push(format!("reference:{}", hash(r.transcription.as_bytes())));
        }
        for key in keys {
            if let Some(&j) = seen.get(&key) {
                let a = root(&parent, i);
                let b = root(&parent, j);
                parent[a] = b;
            } else {
                seen.insert(key, i);
            }
        }
    }
    let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for i in 0..rows.len() {
        groups.entry(root(&parent, i)).or_default().push(i);
    }
    let mut groups: Vec<_> = groups.into_values().collect();
    // Sorting by minimum ID and greedy size balance is deterministic; labels are never consulted.
    groups.sort_by_key(|g| g.iter().map(|&i| rows[i].0.id.as_str()).min().unwrap());
    let mut folds = vec![vec![]; 5];
    for group in groups {
        let f = (0..5).min_by_key(|&f| (folds[f].len(), f)).unwrap();
        folds[f].extend(group);
    }
    if folds.iter().any(Vec::is_empty) {
        return Err("fewer than five independent OOF groups".into());
    }
    Ok(folds)
}
fn make(refs: &[u8], origins: BTreeMap<String, String>, manifest_sha256: String) -> Result<Plan> {
    let rows = parse(refs)?;
    let groups = partitions(&rows, &origins)?;
    let all: BTreeSet<_> = rows.iter().map(|(r, _)| r.id.as_str()).collect();
    let mut folds = Vec::new();
    for (index, group) in groups.into_iter().enumerate() {
        let heldout: BTreeSet<_> = group.iter().map(|&i| rows[i].0.id.as_str()).collect();
        let fit: BTreeSet<_> = all.difference(&heldout).copied().collect();
        folds.push(Fold {
            index,
            heldout_ids: heldout.iter().map(|s| s.to_string()).collect(),
            fit_ids: fit.iter().map(|s| s.to_string()).collect(),
            corpus_sha256: hash(&corpus(&rows, &fit)?),
        });
    }
    Ok(Plan {
        schema_version: "kzn.statistics.oof_plan.v1".into(),
        reference_sha256: hash(refs),
        manifest_sha256,
        origins,
        folds,
        full_corpus_sha256: hash(&corpus(&rows, &all)?),
        full_count: rows.len(),
    })
}
impl Plan {
    pub fn validate(&self, refs: &[u8]) -> Result<()> {
        let expected = make(refs, self.origins.clone(), self.manifest_sha256.clone())?;
        if serde_json::to_value(self)? != serde_json::to_value(expected)? {
            return Err("OOF plan differs from references/groups/complement corpora".into());
        }
        Ok(())
    }
    pub fn bind(&self, report: &SourceReviewReport, training: bool) -> Result<()> {
        let base = &report.base;
        let row = base
            .evidence
            .iter()
            .find(|r| {
                r.kind == RecognitionEvidenceKind::LexicalStatistics
                    && r.status == RecognitionEvidenceStatus::Observed
            })
            .ok_or("OOF statistics missing")?;
        let s: StatisticsObservation =
            serde_json::from_value(row.value.clone().ok_or("OOF observation missing")?)?;
        self.bind_corpus(
            &base.segment_id,
            training,
            &s.corpus_id,
            &s.corpus_sha256,
            s.corpus_document_count,
        )
    }
    fn bind_corpus(
        &self,
        segment: &str,
        training: bool,
        corpus_id: &str,
        corpus_hash: &str,
        count: u64,
    ) -> Result<()> {
        let (expected_id, expected_hash, expected_count) = if training {
            let fold = self
                .folds
                .iter()
                .find(|f| f.heldout_ids.iter().any(|id| id == segment))
                .ok_or("report is not in any heldout fold")?;
            (
                format!("kzn.ocr.oof.fold{}.v1", fold.index),
                fold.corpus_sha256.as_str(),
                fold.fit_ids.len(),
            )
        } else {
            (
                "kzn.ocr.oof.full.v1".into(),
                self.full_corpus_sha256.as_str(),
                self.full_count,
            )
        };
        if corpus_id != expected_id
            || corpus_hash != expected_hash
            || count != expected_count as u64
        {
            return Err("OOF report uses wrong fold/full training corpus".into());
        }
        Ok(())
    }
}
pub(super) fn prepare(refs: &[u8], manifest: &[u8], output: &std::path::Path) -> Result<()> {
    let rows = parse(refs)?;
    let ids: BTreeSet<_> = rows.iter().map(|(r, _)| r.id.as_str()).collect();
    let m: Value = serde_json::from_slice(manifest)?;
    if m["schema_version"] != "kzn.recognition.dataset.v1" {
        return Err("invalid group manifest".into());
    }
    let mut origins = BTreeMap::new();
    for item in m["items"].as_array().ok_or("manifest items missing")? {
        let id = item["id"].as_str().ok_or("manifest id missing")?;
        if !ids.contains(id) {
            continue;
        }
        let reference = &rows.iter().find(|(r, _)| r.id == id).unwrap().0;
        if item["split"] != "train"
            || item["document_id"] != reference.document_id
            || item["source"] != serde_json::to_value(reference.source)?
        {
            return Err("manifest/reference binding mismatch".into());
        }
        if origins
            .insert(
                id.into(),
                item["origin_id"].as_str().ok_or("origin missing")?.into(),
            )
            .is_some()
        {
            return Err("duplicate manifest id".into());
        }
    }
    let plan = make(refs, origins, hash(manifest))?;
    plan.validate(refs)?;
    std::fs::create_dir_all(output)?;
    let all: BTreeSet<_> = rows.iter().map(|(r, _)| r.id.as_str()).collect();
    std::fs::write(output.join("full.clean.jsonl"), corpus(&rows, &all)?)?;
    for fold in &plan.folds {
        let fit: BTreeSet<_> = fold.fit_ids.iter().map(String::as_str).collect();
        std::fs::write(
            output.join(format!("fold{}.clean.jsonl", fold.index)),
            corpus(&rows, &fit)?,
        )?;
        let mut inputs = Vec::new();
        for (r, raw) in rows
            .iter()
            .filter(|(r, _)| fold.heldout_ids.contains(&r.id))
        {
            let projected = json!({"id":r.id,"document_id":r.document_id,"source":r.source,"text":r.text,"engine":raw["engine"],"confidence":raw["confidence"],"confidence_scale":raw["confidence_scale"]});
            serde_json::to_writer(&mut inputs, &projected)?;
            inputs.push(b'\n');
        }
        std::fs::write(
            output.join(format!("fold{}.inputs.jsonl", fold.index)),
            inputs,
        )?;
    }
    std::fs::write(output.join("plan.json"), serde_json::to_vec_pretty(&plan)?)?;
    Ok(())
}
pub(super) fn normalize_signature(signature: &mut Value) {
    signature
        .as_object_mut()
        .unwrap()
        .remove("statistics_asset_id");
    signature
        .as_object_mut()
        .unwrap()
        .remove("statistics_corpus_sha256");
    if let Some(items) = signature["analyzer"].as_array_mut() {
        items.retain(|a| a["component"] != "lexical_statistics");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Vec<u8>, BTreeMap<String, String>, Value) {
        let mut bytes = Vec::new();
        let mut origins = BTreeMap::new();
        let mut items = vec![];
        for i in 0..10 {
            let id = format!("id{i}");
            let doc = format!("doc{i}");
            origins.insert(id.clone(), format!("origin{i}"));
            let row = json!({"id":id,"document_id":doc,"source":"ocr","text":format!("observed{i}"),"transcription":format!("clean{i}"),"transcription_status":"verified","comparison_policy":"raw.v1","split":"train","engine":"test","confidence":null,"confidence_scale":"unknown"});
            serde_json::to_writer(&mut bytes, &row).unwrap();
            bytes.push(b'\n');
            items.push(json!({"id":id,"document_id":doc,"origin_id":format!("origin{i}"),"source":"ocr","split":"train"}));
        }
        (
            bytes,
            origins,
            json!({"schema_version":"kzn.recognition.dataset.v1","items":items}),
        )
    }
    #[test]
    fn transitive_groups_stay_together_and_complements_are_exact() {
        let (refs, mut origins, _) = fixture();
        let mut rows = parse(&refs).unwrap();
        rows[1].0.document_id = rows[0].0.document_id.clone();
        origins.insert("id2".into(), origins["id1"].clone());
        rows[3].0.transcription = rows[2].0.transcription.clone();
        let folds = partitions(&rows, &origins).unwrap();
        assert!(folds.iter().any(|f| (0..4).all(|i| f.contains(&i))));
        assert_eq!(folds, partitions(&rows, &origins).unwrap());
        let p = make(&refs, fixture().1, "manifest".into()).unwrap();
        let all: BTreeSet<_> = (0..10).map(|i| format!("id{i}")).collect();
        let mut held = BTreeSet::new();
        for f in &p.folds {
            let h: BTreeSet<_> = f.heldout_ids.iter().cloned().collect();
            let fit: BTreeSet<_> = f.fit_ids.iter().cloned().collect();
            assert!(h.is_disjoint(&fit));
            assert_eq!(h.union(&fit).cloned().collect::<BTreeSet<_>>(), all);
            for id in h {
                assert!(held.insert(id));
            }
        }
        assert_eq!(held, all);
        p.validate(&refs).unwrap();
        let mut changed = p.clone();
        changed.folds[0].corpus_sha256 = "wrong".into();
        assert!(changed.validate(&refs).is_err());
        let mut changed = p.clone();
        let id = changed.folds[0].heldout_ids[0].clone();
        changed.folds[0].fit_ids.push(id);
        assert!(changed.validate(&refs).is_err());
        origins.remove("id0");
        assert!(partitions(&rows, &origins).is_err());
    }
    #[test]
    fn report_corpus_must_exclude_its_own_fold() {
        let (refs, origins, _) = fixture();
        let p = make(&refs, origins, "manifest".into()).unwrap();
        let f = &p.folds[0];
        let id = &f.heldout_ids[0];
        assert!(p
            .bind_corpus(
                id,
                true,
                "kzn.ocr.oof.fold0.v1",
                &f.corpus_sha256,
                f.fit_ids.len() as u64
            )
            .is_ok());
        assert!(p
            .bind_corpus(
                id,
                true,
                "kzn.ocr.oof.full.v1",
                &p.full_corpus_sha256,
                p.full_count as u64
            )
            .is_err());
        let wrong = &p.folds[1];
        assert!(p
            .bind_corpus(
                id,
                true,
                "kzn.ocr.oof.fold1.v1",
                &wrong.corpus_sha256,
                wrong.fit_ids.len() as u64
            )
            .is_err());
        assert!(p
            .bind_corpus(
                "unknown",
                true,
                "kzn.ocr.oof.fold0.v1",
                &f.corpus_sha256,
                f.fit_ids.len() as u64
            )
            .is_err());
        assert!(p
            .bind_corpus(
                "dev",
                false,
                "kzn.ocr.oof.full.v1",
                &p.full_corpus_sha256,
                p.full_count as u64
            )
            .is_ok());
        assert!(p
            .bind_corpus(
                "dev",
                false,
                "kzn.ocr.oof.fold0.v1",
                &f.corpus_sha256,
                f.fit_ids.len() as u64
            )
            .is_err());
    }
    #[test]
    fn preparation_projects_recognized_text_and_rejects_bad_manifest() {
        let (refs, _, manifest) = fixture();
        let dir = std::env::temp_dir().join(format!("kzn-oof-test-{}", std::process::id()));
        prepare(&refs, &serde_json::to_vec(&manifest).unwrap(), &dir).unwrap();
        let mut count = 0;
        for i in 0..5 {
            let input = std::fs::read_to_string(dir.join(format!("fold{i}.inputs.jsonl"))).unwrap();
            for line in input.lines() {
                let v: Value = serde_json::from_str(line).unwrap();
                assert_eq!(v.as_object().unwrap().len(), 7);
                assert!(v.get("transcription").is_none());
                assert!(v["text"].as_str().unwrap().starts_with("observed"));
                assert!(v["confidence"].is_null());
                count += 1;
            }
        }
        assert_eq!(count, 10);
        for variant in 0..3 {
            let mut bad = manifest.clone();
            match variant {
                0 => {
                    bad["items"].as_array_mut().unwrap().pop();
                }
                1 => {
                    bad["items"][0]["split"] = json!("test");
                }
                _ => {
                    let duplicate = bad["items"][0].clone();
                    bad["items"].as_array_mut().unwrap().push(duplicate);
                }
            }
            assert!(prepare(&refs, &serde_json::to_vec(&bad).unwrap(), &dir).is_err());
        }
        // Only removes this test's known temporary directory.
        std::fs::remove_dir_all(dir).unwrap();
    }
}
