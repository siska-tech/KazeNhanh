use kaze_nhanh::*;
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let out = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or(root.join("target/judge-smoke.json"));
    let reference_count = kaze_nhanh_qwen::verify_tokenizer_references(
        &root.join("target/qwen-judge"),
        &root.join("tests/fixtures/qwen/reference-cases.json"),
    )
    .map_err(|failure| format!("tokenizer reference failed: {failure:?}"))?;
    let factory = Arc::new(QwenNaturalnessFactory::new(QwenJudgeConfig::local(
        root.join("target/qwen-judge"),
    ))?);
    let worker = Arc::new(SecondaryWorker::new(
        factory.clone(),
        SecondaryPolicy {
            timeout: Duration::from_secs(180),
            max_calls: 8,
            ..SecondaryPolicy::default()
        },
    )?);
    let profile = DomainProfile::builtin("ja.asr.v1")?;
    let engine = japanese_engine_with_profile(
        SudachiConfig::from_paths(
            root.join("resources/sudachi/system.dic"),
            root.join("resources/sudachi/sudachi.json"),
            SudachiMode::C,
        )?,
        profile.clone(),
        profile.evaluation_config(),
    )?
    .with_secondary_worker(worker.clone());
    let mut cases = Vec::new();
    for text in [
        "今日は晴れです。",
        "今日は晴れですですです。",
        "資料を確�してください。",
        "これは（途中です。",
        "今日は晴れですですです。",
    ] {
        let mut input = TextInput::new(text);
        input.source = SourceKind::Asr;
        let start = Instant::now();
        let report = engine.evaluate(input)?;
        cases.push(serde_json::json!({"text":text,"elapsed_ms":start.elapsed().as_millis(),"report":report}));
    }
    let long_text = "あ".repeat(4000);
    let bounded = engine.evaluate(TextInput::new(&long_text))?;
    if bounded.routing.status != RoutingStatus::BudgetExceeded
        || bounded.verdict != Verdict::Undetermined
    {
        return Err("context budget did not abstain".into());
    }
    let mut semantic_input = TextInput::new("これは回答です。");
    semantic_input.reference = Some("参照です。");
    let unsupported = engine.evaluate(semantic_input)?;
    if unsupported.routing.status != RoutingStatus::InvalidOutput
        || unsupported.scores.semantic_consistency.value.is_some()
    {
        return Err("unsupported semantics must abstain".into());
    }
    let candidate_low_scores = cases[1..4]
        .iter()
        .filter(|case| {
            case["report"]["scores"]["naturalness"]["value"]
                .as_f64()
                .is_some_and(|v| v < 0.5)
        })
        .count();
    let data = serde_json::json!({"cases":cases,"total_calls":worker.total_calls(),"context_budget_report":bounded,"unsupported_semantic_report":unsupported,"quality_accepted":false,"actual_forwards":factory.total_forwards(),"candidate_count":3,"candidate_low_scores":candidate_low_scores,"experimental":true,"reference_count":reference_count});
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&out, serde_json::to_string_pretty(&data)?)?;
    println!("{}", out.display());
    if factory.total_forwards() != 4 {
        return Err("actual CPU forward count mismatch".into());
    }
    if data["cases"][0]["report"]["metrics"]["slm_calls"] != 0 || data["total_calls"] != 6 {
        return Err("selective call count mismatch".into());
    }
    if data["cases"].as_array().unwrap()[1..4]
        .iter()
        .any(|case| case["report"]["routing"]["status"] != "completed")
    {
        return Err("candidate completion failed".into());
    }
    if data["cases"][1]["report"]["routing"]["status"] != "completed"
        || data["cases"][4]["report"]["scores"] != data["cases"][1]["report"]["scores"]
    {
        return Err("real model completion/repeat mismatch: inspect output".into());
    }
    Ok(())
}
