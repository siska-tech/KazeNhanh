//! Offline, uncalibrated logistic comparison. Never emits RecognitionReport or a low-risk decision.
use kaze_nhanh::{source_adapters::SourceReviewReport, *};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
#[path = "fusion_baseline/oof.rs"]
mod oof;
const N: usize = 8;
const FEATURES: [&str; N] = [
    "confidence",
    "log_scalar_length",
    "oov_fraction",
    "unseen_char_fraction",
    "unseen_word_fraction",
    "unseen_word_pair_fraction",
    "unseen_pos_fraction",
    "class_transition_fraction",
];
#[derive(Deserialize)]
struct Reference {
    id: String,
    document_id: String,
    source: RecognitionSource,
    text: String,
    transcription: String,
    transcription_status: String,
    comparison_policy: String,
    split: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Observation {
    schema_version: String,
    reports: Vec<SourceReviewReport>,
}
struct Row {
    id: String,
    document: String,
    reference: String,
    x: [Option<f64>; N],
    y: f64,
}
struct Data {
    rows: Vec<Row>,
    signature: Value,
}
fn read(path: &std::ffi::OsStr) -> Result<Vec<u8>> {
    if std::fs::metadata(path)?.len() > 64 * 1024 * 1024 {
        return Err("input exceeds 64 MiB".into());
    }
    Ok(std::fs::read(path)?)
}
fn fraction(n: usize, d: usize) -> Option<f64> {
    (d > 0).then(|| n as f64 / d as f64)
}
fn extract(r: &SourceReviewReport) -> Result<([Option<f64>; N], Value)> {
    r.validate()?;
    let b = &r.base;
    let row = b
        .evidence
        .iter()
        .find(|e| {
            e.kind == RecognitionEvidenceKind::LexicalStatistics
                && e.status == RecognitionEvidenceStatus::Observed
        })
        .ok_or("observed statistics required")?;
    let s: StatisticsObservation =
        serde_json::from_value(row.value.clone().ok_or("statistics missing")?)?;
    let source = b
        .recognizer_evidence
        .as_ref()
        .ok_or("source evidence missing")?;
    if source.confidences.len() != 1 {
        return Err("exactly one segment confidence required".into());
    }
    let c = &source.confidences[0];
    if c.granularity != ConfidenceGranularity::Segment {
        return Err("segment confidence required".into());
    }
    let confidence = match &c.score {
        Some(v)
            if v.meaning == ConfidenceMeaning::EngineScore
                && v.direction == ConfidenceDirection::HigherIsBetter
                && v.range == Some([0.0, 1.0]) =>
        {
            Some(v.value)
        }
        Some(_) => return Err("unsupported confidence scale".into()),
        None => None,
    };
    let string = b
        .string_features
        .as_ref()
        .ok_or("string features required")?;
    let pos = s
        .pos
        .as_ref()
        .filter(|p| p.missing_pair_count == 0)
        .and_then(|p| p.frequencies.as_ref())
        .and_then(|f| f.unseen_fraction);
    let x = [
        confidence,
        Some((string.scalar_count as f64).ln_1p()),
        fraction(s.oov_count, s.words.unit_count),
        s.character_pairs.unseen_fraction,
        s.words.unseen_fraction,
        s.word_pairs.unseen_fraction,
        pos,
        fraction(
            string.transition_count,
            string.scalar_count.saturating_sub(1),
        ),
    ];
    let signature = json!({"rule":r.rule,"source":b.source,"domain":b.domain,"statistics_asset_id":s.asset_id,"statistics_corpus_sha256":s.corpus_sha256,"analyzer":b.provenance});
    Ok((x, signature))
}
fn load(refs: &[u8], reports: &[u8], split: &str) -> Result<Data> {
    load_mode(refs, reports, split, None)
}
fn load_mode(refs: &[u8], reports: &[u8], split: &str, plan: Option<&oof::Plan>) -> Result<Data> {
    let input: Observation = serde_json::from_slice(reports)?;
    if input.schema_version != "kzn.recognition.source_review_observation.v1"
        || input.reports.len() > 4096
    {
        return Err("invalid report schema/size".into());
    }
    let mut by_id = BTreeMap::new();
    let mut signature = None;
    for report in input.reports {
        let (x, mut s) = extract(&report)?;
        if let Some(plan) = plan {
            plan.bind(&report, split == "train")?;
            oof::normalize_signature(&mut s);
        }
        if signature.as_ref().is_some_and(|old| old != &s) {
            return Err("mixed feature/source/artifact identities".into());
        }
        signature = Some(s);
        if by_id
            .insert(report.base.segment_id.clone(), (report.base, x))
            .is_some()
        {
            return Err("duplicate report id".into());
        }
    }
    let mut rows = Vec::new();
    for line in std::str::from_utf8(refs)?
        .lines()
        .filter(|l| !l.trim().is_empty())
    {
        if rows.len() >= 4096 {
            return Err("reference limit exceeded".into());
        }
        let r: Reference = serde_json::from_str(line)?;
        if r.split != split
            || r.transcription_status != "verified"
            || r.comparison_policy != "raw.v1"
        {
            return Err("expected declared split, verified reference and raw.v1".into());
        }
        let (b, x) = by_id
            .remove(&r.id)
            .ok_or("missing/duplicate reference id")?;
        if b.original_text != r.text || b.document_id != r.document_id || b.source != r.source {
            return Err("reference/report mismatch".into());
        }
        rows.push(Row {
            id: r.id,
            document: r.document_id,
            y: f64::from(r.text != r.transcription),
            reference: r.transcription,
            x,
        });
    }
    if rows.is_empty() || !by_id.is_empty() {
        return Err("empty or unmatched dataset".into());
    }
    Ok(Data {
        rows,
        signature: signature.ok_or("missing signature")?,
    })
}
fn disjoint(train: &Data, dev: &Data) -> Result<()> {
    if train.signature != dev.signature {
        return Err("train/development feature identity mismatch".into());
    }
    let ids: BTreeSet<_> = train.rows.iter().map(|r| &r.id).collect();
    let docs: BTreeSet<_> = train.rows.iter().map(|r| &r.document).collect();
    let refs: BTreeSet<_> = train
        .rows
        .iter()
        .filter(|r| !r.reference.is_empty())
        .map(|r| &r.reference)
        .collect();
    if dev
        .rows
        .iter()
        .any(|r| ids.contains(&r.id) || docs.contains(&r.document) || refs.contains(&r.reference))
    {
        return Err("train/development group overlap".into());
    }
    Ok(())
}
#[derive(Serialize, PartialEq, Debug)]
struct Model {
    indices: Vec<usize>,
    means: Vec<f64>,
    scales: Vec<f64>,
    weights: Vec<f64>,
}
impl Model {
    fn encode(&self, x: &[Option<f64>; N]) -> Vec<f64> {
        let mut z = vec![1.0];
        for (j, &i) in self.indices.iter().enumerate() {
            z.push(x[i].map_or(0.0, |v| (v - self.means[j]) / self.scales[j]));
            z.push(f64::from(x[i].is_none()));
        }
        z
    }
    fn margin(&self, x: &[Option<f64>; N]) -> f64 {
        self.encode(x)
            .iter()
            .zip(&self.weights)
            .map(|(v, w)| v * w)
            .sum()
    }
}
fn sigmoid(x: f64) -> f64 {
    if x >= 0.0 {
        1.0 / (1.0 + (-x).exp())
    } else {
        let e = x.exp();
        e / (1.0 + e)
    }
}
fn fit(rows: &[Row], indices: Vec<usize>) -> Result<Model> {
    if rows.len() < 2 || !rows.iter().any(|r| r.y == 0.0) || !rows.iter().any(|r| r.y == 1.0) {
        return Err("training requires both classes".into());
    }
    let mut m = Model {
        weights: vec![0.0; 1 + indices.len() * 2],
        means: vec![],
        scales: vec![],
        indices,
    };
    for &i in &m.indices {
        let values: Vec<_> = rows.iter().filter_map(|r| r.x[i]).collect();
        if values.iter().any(|v| !v.is_finite()) {
            return Err("nonfinite feature".into());
        }
        let mean = if values.is_empty() {
            0.0
        } else {
            values.iter().sum::<f64>() / values.len() as f64
        };
        let scale = if values.is_empty() {
            1.0
        } else {
            (values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64).sqrt()
        };
        m.means.push(mean);
        m.scales.push(if scale < 1e-12 { 1.0 } else { scale });
    }
    let encoded: Vec<_> = rows.iter().map(|r| m.encode(&r.x)).collect();
    // Fixed before development: unweighted full-batch logistic loss, L2=0.01 except intercept.
    for _ in 0..500 {
        let mut grad = vec![0.0; m.weights.len()];
        for (z, row) in encoded.iter().zip(rows) {
            let margin = z.iter().zip(&m.weights).map(|(v, w)| v * w).sum();
            let error = sigmoid(margin) - row.y;
            for (g, v) in grad.iter_mut().zip(z) {
                *g += error * v / rows.len() as f64;
            }
        }
        for (i, (w, g)) in m.weights.iter_mut().zip(grad).enumerate() {
            *w -= 0.05 * (g + if i == 0 { 0.0 } else { 0.01 * *w });
        }
    }
    if m.weights.iter().any(|v| !v.is_finite()) {
        return Err("nonfinite fitted weights".into());
    }
    Ok(m)
}
fn evaluate(model: &Model, rows: &[Row]) -> Value {
    let (mut tp, mut fp, mut tn, mut fn_) = (0, 0, 0, 0);
    let mut outputs = Vec::new();
    for row in rows {
        let margin = model.margin(&row.x);
        let flag = margin >= 0.0;
        match (flag, row.y == 1.0) {
            (true, true) => tp += 1,
            (true, false) => fp += 1,
            (false, true) => fn_ += 1,
            (false, false) => tn += 1,
        }
        outputs.push(json!({"id":row.id,"uncalibrated_margin":margin,"flagged":flag}));
    }
    json!({"flagged_matches":fp,"flagged_mismatches":tp,"unflagged_matches":tn,"unflagged_mismatches":fn_,"precision":fraction(tp,tp+fp),"recall":fraction(tp,tp+fn_),"false_flag_rate":fraction(fp,fp+tn),"cases":outputs})
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() == 4 && args[0] == "prepare-oof" {
        return oof::prepare(
            &read(&args[1])?,
            &read(&args[2])?,
            std::path::Path::new(&args[3]),
        );
    }
    if args.first().is_some_and(|v| v == "merge-oof") {
        if args.len() != 7 {
            return Err(
                "Usage: fusion_baseline merge-oof output.json fold0.json ... fold4.json".into(),
            );
        }
        let mut combined = Observation {
            schema_version: "kzn.recognition.source_review_observation.v1".into(),
            reports: vec![],
        };
        let mut ids = BTreeSet::new();
        for path in &args[2..] {
            let part: Observation = serde_json::from_slice(&read(path)?)?;
            if part.schema_version != combined.schema_version
                || combined.reports.len() + part.reports.len() > 4096
            {
                return Err("invalid merged schema/size".into());
            }
            for report in part.reports {
                report.validate()?;
                if !ids.insert(report.base.segment_id.clone()) {
                    return Err("duplicate fold report".into());
                }
                combined.reports.push(report);
            }
        }
        std::fs::write(&args[1], serde_json::to_vec(&combined)?)?;
        return Ok(());
    }
    if args.len() != 5 && !(args.len() == 7 && args[5] == "--oof-plan") {
        return Err("Usage: fusion_baseline train.references.jsonl train.reports.json development.references.jsonl development.reports.json output.json [--oof-plan plan.json]".into());
    }
    let bytes: Vec<_> = args[..4].iter().map(|p| read(p)).collect::<Result<_>>()?;
    let plan: Option<oof::Plan> = if args.len() == 7 {
        let p: oof::Plan = serde_json::from_slice(&read(&args[6])?)?;
        p.validate(&bytes[0])?;
        Some(p)
    } else {
        None
    };
    let train = if let Some(p) = &plan {
        load_mode(&bytes[0], &bytes[1], "train", Some(p))?
    } else {
        load(&bytes[0], &bytes[1], "train")?
    };
    // Fit all ablations before loading development labels or features.
    let models = [
        ("confidence_only", fit(&train.rows, vec![0])?),
        ("text_only", fit(&train.rows, (1..N).collect())?),
        ("integrated", fit(&train.rows, (0..N).collect())?),
    ];
    let dev = load_mode(&bytes[2], &bytes[3], "development", plan.as_ref())?;
    disjoint(&train, &dev)?;
    let rows:Vec<_>=models.into_iter().map(|(name,model)|json!({"ablation":name,"development":evaluate(&model,&dev.rows),"model":model})).collect();
    let out = json!({"schema_version":"kzn.fusion.development.v1","quality_accepted":false,"runtime_policy_changed":false,"oof_plan":plan,"method":"standardized_logistic_l2.fixed500.v1","steps":500,"learning_rate":0.05,"l2":0.01,"flag_margin_threshold":0.0,"features":FEATURES,"missingness":"train observed mean imputation plus missing indicator per feature","input_sha256":bytes.iter().map(|b|format!("{:x}",Sha256::digest(b))).collect::<Vec<_>>(),"feature_signature":train.signature,"training_count":train.rows.len(),"development_count":dev.rows.len(),"results":rows,"limitations":["uncalibrated margins, not recognition error probabilities","development only; no acceptance, calibration or low-risk decision","text statistics use train references only; OOF is applied only when oof_plan is present","declared/exact group audit does not establish independence of templates"]});
    let path = std::path::Path::new(&args[4]);
    if let Some(p) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(p)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(&out)?)?;
    println!(
        "{}",
        json!({"training_count":train.rows.len(),"development_count":dev.rows.len(),"quality_accepted":false})
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn row(id: &str, value: Option<f64>, y: f64) -> Row {
        Row {
            id: id.into(),
            document: format!("doc-{id}"),
            reference: format!("ref-{id}"),
            x: [value; N],
            y,
        }
    }
    #[test]
    fn deterministic_training_and_direction_are_not_label_or_test_dependent() {
        let rows = vec![
            row("a", Some(-2.0), 0.0),
            row("b", Some(-1.0), 0.0),
            row("c", Some(1.0), 1.0),
            row("d", Some(2.0), 1.0),
        ];
        let model = fit(&rows, vec![0]).unwrap();
        assert_eq!(model, fit(&rows, vec![0]).unwrap());
        assert!(model.margin(&rows[0].x) < 0.0);
        assert!(model.margin(&rows[3].x) > 0.0);
        let before = serde_json::to_vec(&model).unwrap();
        let dev = vec![row("e", Some(1000.0), 0.0), row("f", None, 1.0)];
        let result = evaluate(&model, &dev);
        assert_eq!(result["flagged_matches"], 1);
        assert_eq!(serde_json::to_vec(&model).unwrap(), before);
        assert_eq!(model.means, vec![0.0]);
    }
    #[test]
    fn missing_is_mean_imputed_with_separate_indicator_and_constant_scale_is_safe() {
        let rows = vec![row("a", Some(2.0), 0.0), row("b", None, 1.0)];
        let model = fit(&rows, vec![0]).unwrap();
        assert_eq!(model.means, vec![2.0]);
        assert_eq!(model.scales, vec![1.0]);
        assert_eq!(model.encode(&[None; N]), vec![1.0, 0.0, 1.0]);
        assert_eq!(model.encode(&[Some(2.0); N]), vec![1.0, 0.0, 0.0]);
        let all_missing = fit(&[row("a", None, 0.0), row("b", None, 1.0)], vec![0]).unwrap();
        assert!(all_missing.margin(&[None; N]).is_finite());
        assert!(fit(&[row("a", Some(1.0), 0.0)], vec![0]).is_err());
        assert!(fit(
            &[row("a", Some(f64::NAN), 0.0), row("b", Some(1.0), 1.0)],
            vec![0]
        )
        .is_err());
    }
    #[test]
    fn holdout_groups_and_feature_identities_must_be_disjoint_and_equal_respectively() {
        let train = Data {
            rows: vec![row("train", Some(1.0), 1.0)],
            signature: json!("same"),
        };
        let mut dev = Data {
            rows: vec![row("dev", Some(1.0), 1.0)],
            signature: json!("same"),
        };
        disjoint(&train, &dev).unwrap();
        dev.rows[0].reference = train.rows[0].reference.clone();
        assert!(disjoint(&train, &dev).is_err());
        dev.rows[0].reference = "other".into();
        dev.rows[0].document = train.rows[0].document.clone();
        assert!(disjoint(&train, &dev).is_err());
        dev.rows[0].document = "other".into();
        dev.signature = json!("different");
        assert!(disjoint(&train, &dev).is_err());
    }
    #[test]
    fn empty_or_forged_input_does_not_produce_metrics() {
        assert!(load(
            b"",
            br#"{"schema_version":"unknown","reports":[]}"#,
            "train"
        )
        .is_err());
        assert!(load(
            b"",
            br#"{"schema_version":"kzn.recognition.source_review_observation.v1","reports":[]}"#,
            "train"
        )
        .is_err());
        let model = Model {
            indices: vec![],
            means: vec![],
            scales: vec![],
            weights: vec![-1.0],
        };
        let result = evaluate(&model, &[row("one", None, 1.0)]);
        assert!(result["precision"].is_null());
        assert_eq!(result["recall"], 0.0);
        assert_eq!(result["unflagged_mismatches"], 1);
    }
}
