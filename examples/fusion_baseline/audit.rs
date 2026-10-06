//! Read-only audit of an already fitted development artifact; never fits or changes decisions.
use super::*;
#[derive(Clone, Copy)]
enum Slice {
    All,
    ObservedNonempty,
    MissingNonempty,
    ObservedEmpty,
    MissingEmpty,
}
impl Slice {
    fn name(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::ObservedNonempty => "observed_confidence_nonempty",
            Self::MissingNonempty => "missing_confidence_nonempty",
            Self::ObservedEmpty => "observed_confidence_empty",
            Self::MissingEmpty => "missing_confidence_empty",
        }
    }
    fn includes(self, r: &Row) -> bool {
        let observed = r.x[0].is_some();
        let empty = r.x[1] == Some(0.0);
        match self {
            Self::All => true,
            Self::ObservedNonempty => observed && !empty,
            Self::MissingNonempty => !observed && !empty,
            Self::ObservedEmpty => observed && empty,
            Self::MissingEmpty => !observed && empty,
        }
    }
}
fn model(value: &Value) -> Result<Model> {
    let m: Model = serde_json::from_value(value.clone())?;
    if m.indices.is_empty()
        || m.indices.iter().any(|&i| i >= N)
        || m.indices.iter().collect::<BTreeSet<_>>().len() != m.indices.len()
        || m.means.len() != m.indices.len()
        || m.scales.len() != m.indices.len()
        || m.weights.len() != 1 + 2 * m.indices.len()
        || m.means
            .iter()
            .chain(&m.scales)
            .chain(&m.weights)
            .any(|v| !v.is_finite())
        || m.scales.iter().any(|v| *v <= 0.0)
    {
        return Err("invalid frozen model".into());
    }
    Ok(m)
}
fn summarize(m: &Model, rows: &[Row], slice: Slice) -> Value {
    let (mut tp, mut fp, mut tn, mut miss) = (0, 0, 0, 0);
    for r in rows.iter().filter(|r| slice.includes(r)) {
        match (m.margin(&r.x) >= 0.0, r.y == 1.0) {
            (true, true) => tp += 1,
            (true, false) => fp += 1,
            (false, false) => tn += 1,
            (false, true) => miss += 1,
        }
    }
    json!({"slice":slice.name(),"count":tp+fp+tn+miss,"matches":fp+tn,"mismatches":tp+miss,"flagged_matches":fp,"flagged_mismatches":tp,"unflagged_matches":tn,"unflagged_mismatches":miss,"precision":fraction(tp,tp+fp),"recall":fraction(tp,tp+miss),"false_flag_rate":fraction(fp,fp+tn)})
}
pub(super) fn run(args: &[std::ffi::OsString]) -> Result<()> {
    if args.len() != 5 {
        return Err("Usage: fusion_baseline audit-frozen frozen.json development.refs.jsonl development.reports.json output.json".into());
    }
    let frozen_bytes = read(&args[1])?;
    let frozen: Value = serde_json::from_slice(&frozen_bytes)?;
    let refs = read(&args[2])?;
    let reports = read(&args[3])?;
    if frozen["schema_version"] != "kzn.fusion.development.v1"
        || frozen["method"] != "standardized_logistic_l2.fixed500.v1"
        || frozen["flag_margin_threshold"] != 0.0
        || frozen["features"] != json!(FEATURES)
        || frozen["input_sha256"][2] != format!("{:x}", Sha256::digest(&refs))
        || frozen["input_sha256"][3] != format!("{:x}", Sha256::digest(&reports))
    {
        return Err("frozen method or development binding mismatch".into());
    }
    let plan: oof::Plan = serde_json::from_value(frozen["oof_plan"].clone())?;
    let data = load_mode(&refs, &reports, "development", Some(&plan))?;
    if data.signature != frozen["feature_signature"] {
        return Err("frozen feature identity mismatch".into());
    }
    let ablations = frozen["results"].as_array().ok_or("missing ablations")?;
    if ablations.len() != 3 {
        return Err("expected three ablations".into());
    }
    let mut output = vec![];
    for (i, a) in ablations.iter().enumerate() {
        let expected = ["confidence_only", "text_only", "integrated"][i];
        let m = model(&a["model"])?;
        let indices = match i {
            0 => vec![0],
            1 => (1..N).collect(),
            _ => (0..N).collect(),
        };
        if a["ablation"] != expected
            || m.indices != indices
            || evaluate(&m, &data.rows) != a["development"]
        {
            return Err("frozen predictions/summary differ from replay".into());
        }
        let slices: Vec<_> = [
            Slice::All,
            Slice::ObservedNonempty,
            Slice::MissingNonempty,
            Slice::ObservedEmpty,
            Slice::MissingEmpty,
        ]
        .into_iter()
        .map(|s| summarize(&m, &data.rows, s))
        .collect();
        output.push(json!({"ablation":expected,"slices":slices}));
    }
    std::fs::write(
        &args[4],
        serde_json::to_vec_pretty(
            &json!({"schema_version":"kzn.fusion.scope_audit.v1","frozen_sha256":format!("{:x}",Sha256::digest(&frozen_bytes)),"quality_accepted":false,"refit":false,"runtime_policy_changed":false,"scope":"same development inputs; unflagged is not low_risk; empty reference/source coverage not established","results":output}),
        )?,
    )?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn slices_partition_missing_empty_and_zero_without_normalizing_away_missingness() {
        let m = Model {
            indices: vec![0],
            means: vec![0.0],
            scales: vec![1.0],
            weights: vec![0.0, 1.0, 0.0],
        };
        let rows: Vec<_> = [(None, 0.0), (None, 1.0), (Some(0.0), 0.0), (Some(0.0), 1.0)]
            .into_iter()
            .enumerate()
            .map(|(i, (c, len))| {
                let mut x = [None; N];
                x[0] = c;
                x[1] = Some(len);
                Row {
                    id: i.to_string(),
                    document: i.to_string(),
                    reference: "gold".into(),
                    x,
                    y: 1.0,
                }
            })
            .collect();
        for s in [
            Slice::ObservedNonempty,
            Slice::MissingNonempty,
            Slice::ObservedEmpty,
            Slice::MissingEmpty,
        ] {
            let v = summarize(&m, &rows, s);
            assert_eq!(v["count"], 1);
            assert!(v["false_flag_rate"].is_null());
        }
        assert_eq!(summarize(&m, &rows, Slice::All)["count"], 4);
        assert!(summarize(&m, &[], Slice::All)["recall"].is_null());
    }
    #[test]
    fn malformed_frozen_models_are_rejected() {
        let base = json!({"indices":[0],"means":[0.0],"scales":[1.0],"weights":[0.0,1.0,0.0]});
        assert!(model(&base).is_ok());
        for (key, value) in [
            ("indices", json!([8])),
            ("indices", json!([0, 0])),
            ("scales", json!([0.0])),
            ("weights", json!([1.0])),
            ("means", json!([])),
        ] {
            let mut v = base.clone();
            v[key] = value;
            assert!(model(&v).is_err());
        }
    }
}
