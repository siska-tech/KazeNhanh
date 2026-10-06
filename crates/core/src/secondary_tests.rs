use super::*;
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc, Condvar, Mutex,
    },
    time::{Duration, Instant},
};
struct EmptyAnalyzer;
impl MorphAnalyzer for EmptyAnalyzer {
    fn analyze(&self, _: &str) -> Result<MorphAnalysis, EvaluationError> {
        Ok(MorphAnalysis::default())
    }
}
#[derive(Clone, Copy)]
enum Behavior {
    Good,
    Error,
    Panic,
    InvalidSpan,
    MissingScore,
    UnexpectedScore,
    LowSemantic,
}
#[derive(Default)]
struct Gate {
    open: Mutex<bool>,
    wake: Condvar,
}
impl Gate {
    fn wait(&self) {
        let mut open = self.open.lock().unwrap();
        while !*open {
            open = self.wake.wait(open).unwrap();
        }
    }
    fn release(&self) {
        *self.open.lock().unwrap() = true;
        self.wake.notify_all();
    }
}
struct Factory {
    loads: Arc<AtomicUsize>,
    calls: Arc<AtomicUsize>,
    behavior: Behavior,
    load_gate: Option<Arc<Gate>>,
    judge_gate: Option<Arc<Gate>>,
    started: Option<mpsc::Sender<()>>,
    load_error: bool,
}
impl SecondaryJudgeFactory for Factory {
    fn artifacts(&self) -> Vec<ArtifactIdentity> {
        vec![ArtifactIdentity {
            component: "test_judge".into(),
            id: "explicit-fake.v1".into(),
            sha256: None,
        }]
    }
    fn load(&self, _: Instant) -> Result<Box<dyn SecondaryJudge>, SecondaryFailure> {
        self.loads.fetch_add(1, Ordering::SeqCst);
        if let Some(gate) = &self.load_gate {
            gate.wait();
        }
        if self.load_error {
            return Err(SecondaryFailure::BackendError);
        }
        Ok(Box::new(Judge {
            calls: self.calls.clone(),
            behavior: self.behavior,
            gate: self.judge_gate.clone(),
            started: self.started.clone(),
        }))
    }
}
struct Judge {
    calls: Arc<AtomicUsize>,
    behavior: Behavior,
    gate: Option<Arc<Gate>>,
    started: Option<mpsc::Sender<()>>,
}
impl SecondaryJudge for Judge {
    fn judge(
        &mut self,
        request: &SecondaryRequest,
    ) -> Result<SecondaryEvaluation, SecondaryFailure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if let Some(started) = &self.started {
            let _ = started.send(());
        }
        if let Some(gate) = &self.gate {
            gate.wait();
        }
        assert!(request.max_generated_tokens > 0);
        let mut output = SecondaryEvaluation {
            naturalness: request
                .dimensions
                .contains(&Dimension::Naturalness)
                .then(|| UnitScore::new(0.9).unwrap()),
            semantic_consistency: request
                .dimensions
                .contains(&Dimension::SemanticConsistency)
                .then(|| UnitScore::new(0.9).unwrap()),
            issues: vec![],
        };
        match self.behavior {
            Behavior::Good => {}
            Behavior::Error => return Err(SecondaryFailure::BackendError),
            Behavior::Panic => panic!("explicit fixture panic"),
            Behavior::MissingScore => output.naturalness = None,
            Behavior::UnexpectedScore => {
                output.semantic_consistency = Some(UnitScore::new(1.0).unwrap())
            }
            Behavior::LowSemantic => {
                output.semantic_consistency = Some(UnitScore::new(0.1).unwrap())
            }
            Behavior::InvalidSpan => output.issues.push(Issue {
                code: "bad_span".into(),
                severity: Severity::Warning,
                span: serde_json::from_str(r#"{"start":1,"end":2}"#).unwrap(),
                stage: DetectionStage::Secondary,
                evidence: serde_json::Value::Null,
                explanation: "bad fixture".into(),
            }),
        }
        Ok(output)
    }
}
fn factory(behavior: Behavior) -> Factory {
    Factory {
        loads: Arc::new(AtomicUsize::new(0)),
        calls: Arc::new(AtomicUsize::new(0)),
        behavior,
        load_gate: None,
        judge_gate: None,
        started: None,
        load_error: false,
    }
}
fn engine(profile: DomainProfile, worker: Arc<SecondaryWorker>) -> EvaluationEngine {
    EvaluationEngine::new(Arc::new(EmptyAnalyzer), profile.evaluation_config())
        .unwrap()
        .with_primary_detector(Arc::new(PrimaryRules::new(profile).unwrap()))
        .with_secondary_worker(worker)
}
fn worker(factory: Factory, policy: SecondaryPolicy) -> Arc<SecondaryWorker> {
    Arc::new(SecondaryWorker::new(Arc::new(factory), policy).unwrap())
}
#[test]
fn normal_invalid_and_missing_reference_do_not_load_or_call_model() {
    let f = factory(Behavior::Good);
    let loads = f.loads.clone();
    let calls = f.calls.clone();
    let worker = worker(f, SecondaryPolicy::default());
    let normal = engine(DomainProfile::screening(), worker.clone())
        .evaluate(TextInput::new("今日は晴れです。"))
        .unwrap();
    assert_eq!(normal.verdict, Verdict::Acceptable);
    assert_eq!(normal.routing.status, RoutingStatus::NotRequested);
    let invalid = engine(
        DomainProfile::builtin("ja.form.v1").unwrap(),
        worker.clone(),
    )
    .evaluate(TextInput::new(""))
    .unwrap();
    assert_eq!(invalid.verdict, Verdict::Invalid);
    let missing = engine(DomainProfile::builtin("ja.llm.v1").unwrap(), worker)
        .evaluate(TextInput::new("これは回答です。"))
        .unwrap();
    assert_eq!(missing.verdict, Verdict::Undetermined);
    assert_eq!(missing.routing.status, RoutingStatus::ContextMissing);
    assert_eq!(
        missing.scores.semantic_consistency.status,
        ScoreStatus::InsufficientContext
    );
    assert_eq!(loads.load(Ordering::SeqCst), 0);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}
#[test]
fn selected_calls_share_one_lazy_model_preserve_warnings_and_record_model_scores() {
    let f = factory(Behavior::Good);
    let loads = f.loads.clone();
    let worker = worker(f, SecondaryPolicy::default());
    let engine = engine(DomainProfile::screening(), worker.clone());
    for _ in 0..2 {
        let report = engine.evaluate(TextInput::new("🙂ああああああ")).unwrap();
        assert_eq!(report.routing.status, RoutingStatus::Completed);
        assert_eq!(report.metrics.slm_calls, 1);
        assert_eq!(report.verdict, Verdict::Suspicious); // SLM cannot silently erase primary evidence.
        assert_eq!(report.scores.naturalness.method, Some(ScoreMethod::Model));
        assert_eq!(report.scores.naturalness.calibration_id, None);
        assert_eq!(report.original_text, "🙂ああああああ");
        assert!(report.provenance.iter().any(|a| a.id == "explicit-fake.v1"));
        report.validate().unwrap();
    }
    assert_eq!(loads.load(Ordering::SeqCst), 1);
    assert_eq!(worker.total_calls(), 2);
}
#[test]
fn reference_judge_can_assess_or_flag_semantics_without_generation_or_validity_override() {
    for behavior in [Behavior::Good, Behavior::LowSemantic] {
        let engine = engine(
            DomainProfile::builtin("ja.llm.v1").unwrap(),
            worker(factory(behavior), SecondaryPolicy::default()),
        );
        let mut input = TextInput::new("料金は100円です。");
        input.reference = Some("料金は200円です。");
        let report = engine.evaluate(input).unwrap();
        assert_eq!(
            report.scores.semantic_consistency.scope,
            ScoreScope::Reference
        );
        assert_eq!(
            report.scores.semantic_consistency.method,
            Some(ScoreMethod::Model)
        );
        assert_eq!(report.scores.validity.method, Some(ScoreMethod::Heuristic));
        assert_eq!(
            report.verdict,
            if matches!(behavior, Behavior::Good) {
                Verdict::Acceptable
            } else {
                Verdict::Suspicious
            }
        );
        let json = serde_json::to_value(report).unwrap();
        assert!(json.get("correction").is_none());
    } // Explicit fake exercises protocol, NOT Japanese semantic accuracy.
}
#[test]
fn invalid_output_backend_error_and_panic_are_abstentions_and_calls_counted() {
    for (behavior, status) in [
        (Behavior::InvalidSpan, RoutingStatus::InvalidOutput),
        (Behavior::MissingScore, RoutingStatus::InvalidOutput),
        (Behavior::UnexpectedScore, RoutingStatus::InvalidOutput),
        (Behavior::Error, RoutingStatus::BackendError),
        (Behavior::Panic, RoutingStatus::BackendError),
    ] {
        let engine = engine(
            DomainProfile::screening(),
            worker(factory(behavior), SecondaryPolicy::default()),
        );
        let report = engine.evaluate(TextInput::new("ああああああ")).unwrap();
        assert_eq!(report.verdict, Verdict::Undetermined);
        assert_eq!(report.routing.status, status);
        assert_eq!(report.metrics.slm_calls, 1);
        assert_eq!(report.scores.naturalness.status, ScoreStatus::Failed);
        assert!(report.issues.iter().any(|i| i.code == "repeated_character"));
        report.validate().unwrap();
    }
    for json in [
        r#"{"naturalness":2,"semantic_consistency":null,"issues":[]}"#,
        r#"{"naturalness":1,"semantic_consistency":null,"issues":[],"correction":"変更"}"#,
        "not JSON",
    ] {
        assert!(SecondaryEvaluation::from_json(json).is_err());
    }
}
#[test]
fn shared_call_and_byte_budgets_reject_before_loading_and_never_reset_per_input() {
    let f = factory(Behavior::Good);
    let loads = f.loads.clone();
    let policy = SecondaryPolicy {
        max_calls: 1,
        max_request_bytes: 24,
        ..SecondaryPolicy::default()
    };
    let worker = worker(f, policy);
    let engine = engine(DomainProfile::screening(), worker.clone());
    let oversized = engine
        .evaluate(TextInput::new("あ".repeat(10).as_str()))
        .unwrap();
    assert_eq!(oversized.routing.status, RoutingStatus::BudgetExceeded);
    assert_eq!(oversized.metrics.slm_calls, 0);
    assert_eq!(loads.load(Ordering::SeqCst), 0);
    assert_eq!(
        engine
            .evaluate(TextInput::new("ああああああ"))
            .unwrap()
            .metrics
            .slm_calls,
        1
    );
    let exhausted = engine.evaluate(TextInput::new("ああああああ")).unwrap();
    assert_eq!(exhausted.routing.status, RoutingStatus::BudgetExceeded);
    assert_eq!(exhausted.metrics.slm_calls, 0);
    assert_eq!(worker.total_calls(), 1);
}
#[test]
fn failed_load_is_cached_and_never_claims_an_inference_call() {
    let mut f = factory(Behavior::Good);
    f.load_error = true;
    let loads = f.loads.clone();
    let engine = engine(
        DomainProfile::screening(),
        worker(f, SecondaryPolicy::default()),
    );
    for _ in 0..2 {
        let report = engine.evaluate(TextInput::new("ああああああ")).unwrap();
        assert_eq!(report.routing.status, RoutingStatus::BackendError);
        assert_eq!(report.metrics.slm_calls, 0);
    }
    assert_eq!(loads.load(Ordering::SeqCst), 1);
}
#[test]
fn cold_load_timeout_cancels_pending_inference_and_warm_request_reuses_model() {
    let gate = Arc::new(Gate::default());
    let mut f = factory(Behavior::Good);
    f.load_gate = Some(gate.clone());
    let calls = f.calls.clone();
    let worker = worker(
        f,
        SecondaryPolicy {
            timeout: Duration::from_millis(200),
            ..SecondaryPolicy::default()
        },
    );
    let engine = engine(DomainProfile::screening(), worker);
    let timed_out = engine.evaluate(TextInput::new("ああああああ")).unwrap();
    assert_eq!(timed_out.routing.status, RoutingStatus::Timeout);
    assert_eq!(timed_out.metrics.slm_calls, 0);
    gate.release();
    let warm = engine.evaluate(TextInput::new("ああああああ")).unwrap();
    assert_eq!(warm.routing.status, RoutingStatus::Completed);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
#[test]
fn busy_worker_has_bounded_queue_expired_requests_never_start_and_timeout_counts_active_call() {
    let gate = Arc::new(Gate::default());
    let mut f = factory(Behavior::Good);
    f.judge_gate = Some(gate.clone());
    let (started, observed) = mpsc::channel();
    f.started = Some(started);
    let calls = f.calls.clone();
    let engine = Arc::new(engine(
        DomainProfile::screening(),
        worker(
            f,
            SecondaryPolicy {
                timeout: Duration::from_millis(200),
                ..SecondaryPolicy::default()
            },
        ),
    ));
    let first_engine = engine.clone();
    let first = std::thread::spawn(move || {
        first_engine
            .evaluate(TextInput::new("ああああああ"))
            .unwrap()
    });
    observed.recv_timeout(Duration::from_secs(3)).unwrap();
    let queued = engine.evaluate(TextInput::new("ああああああ")).unwrap();
    assert_eq!(queued.routing.status, RoutingStatus::Timeout);
    assert_eq!(queued.metrics.slm_calls, 0);
    let saturated = engine.evaluate(TextInput::new("ああああああ")).unwrap();
    assert_eq!(saturated.routing.status, RoutingStatus::BudgetExceeded);
    assert_eq!(saturated.metrics.slm_calls, 0);
    let first = first.join().unwrap();
    assert_eq!(first.routing.status, RoutingStatus::Timeout);
    assert_eq!(first.metrics.slm_calls, 1);
    gate.release();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn report_rejects_forged_completed_or_skipped_calls_and_schema_v2() {
    let engine = engine(
        DomainProfile::screening(),
        worker(factory(Behavior::Good), SecondaryPolicy::default()),
    );
    let report = engine.evaluate(TextInput::new("ああああああ")).unwrap();
    let mut bad = report.clone();
    bad.metrics.slm_calls = 0;
    assert!(bad.validate().is_err());
    let mut bad = report.clone();
    bad.routing.status = RoutingStatus::ContextMissing;
    assert!(bad.validate().is_err());
    let mut bad = report.clone();
    bad.routing.secondary_needed = Some(false);
    assert!(bad.validate().is_err());
    let mut bad = report;
    bad.schema_version = "kzn.evaluation.v2".into();
    assert!(bad.validate().is_err());
}
