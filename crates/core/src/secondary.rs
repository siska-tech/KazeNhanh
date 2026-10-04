//! Optional, bounded secondary execution. No model dependency or automatic download.
use crate::*;
use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{
        atomic::{AtomicU8, AtomicUsize, Ordering},
        mpsc, Arc,
    },
    time::{Duration, Instant},
};

#[derive(Clone, Debug)]
pub struct SecondaryPolicy {
    /// Total calls across all engines sharing this worker, including failed calls.
    pub max_calls: usize,
    pub max_request_bytes: usize,
    pub max_generated_tokens: usize,
    /// Includes queue wait, cold load, inference and validation.
    pub timeout: Duration,
    /// Waiting requests in addition to the one active request.
    pub queue_capacity: usize,
    /// Provisional decision thresholds, never calibrated probabilities.
    pub naturalness_threshold: UnitScore,
    pub semantic_threshold: UnitScore,
}
impl Default for SecondaryPolicy {
    fn default() -> Self {
        Self {
            max_calls: 1000,
            max_request_bytes: 16_384,
            max_generated_tokens: 256,
            timeout: Duration::from_secs(30),
            queue_capacity: 1,
            naturalness_threshold: UnitScore::new(0.5).unwrap(),
            semantic_threshold: UnitScore::new(0.5).unwrap(),
        }
    }
}
/// Owned request for the isolated worker; text remains unmodified.
#[derive(Clone, Debug)]
pub struct SecondaryRequest {
    pub text: String,
    pub reference: Option<String>,
    pub profile_id: String,
    pub primary_issues: Vec<Issue>,
    pub dimensions: Vec<Dimension>,
    pub semantic_scope: ScoreScope,
    pub deadline: Instant,
    pub max_generated_tokens: usize,
}
/// Model scores are uncalibrated; validity is owned by explicit primary constraints.
/// No verdict/correction field. Unknown fields are rejected when parsing JSON.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecondaryEvaluation {
    pub naturalness: Option<UnitScore>,
    pub semantic_consistency: Option<UnitScore>,
    pub issues: Vec<Issue>,
}
impl SecondaryEvaluation {
    pub fn from_json(json: &str) -> Result<Self, SecondaryFailure> {
        if json.len() > 65_536 {
            return Err(SecondaryFailure::InvalidOutput);
        }
        serde_json::from_str(json).map_err(|_| SecondaryFailure::InvalidOutput)
    }
    fn validate(&self, request: &SecondaryRequest) -> Result<(), SecondaryFailure> {
        if self.naturalness.is_some() != request.dimensions.contains(&Dimension::Naturalness)
            || self.semantic_consistency.is_some()
                != request.dimensions.contains(&Dimension::SemanticConsistency)
            || self.issues.len() > 256
            || self.issues.iter().any(|issue| {
                issue.stage != DetectionStage::Secondary
                    || issue.code.trim().is_empty()
                    || issue.code.len() > 128
                    || issue.explanation.trim().is_empty()
                    || issue.explanation.len() > 4096
                    || issue.evidence.to_string().len() > 8192
                    || issue.span.validate(&request.text).is_err()
            })
        {
            return Err(SecondaryFailure::InvalidOutput);
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecondaryFailure {
    BudgetExceeded,
    Timeout,
    BackendError,
    InvalidOutput,
}
/// Implementations must check deadline/cancellation between load/forward steps.
/// A synchronous backend cannot be forcibly killed by the core.
pub trait SecondaryJudge: Send {
    fn judge(
        &mut self,
        request: &SecondaryRequest,
    ) -> Result<SecondaryEvaluation, SecondaryFailure>;
}
pub trait SecondaryJudgeFactory: Send + Sync + 'static {
    fn artifacts(&self) -> Vec<ArtifactIdentity>;
    fn load(&self, deadline: Instant) -> Result<Box<dyn SecondaryJudge>, SecondaryFailure>;
}
struct Work {
    request: SecondaryRequest,
    state: Arc<AtomicU8>, // 0 queued/loading, 1 inference started, 2 cancelled
    response: mpsc::SyncSender<Result<SecondaryEvaluation, SecondaryFailure>>,
}
pub struct SecondaryWorker {
    sender: mpsc::SyncSender<Work>,
    policy: SecondaryPolicy,
    calls: Arc<AtomicUsize>,
    artifacts: Vec<ArtifactIdentity>,
}
pub(crate) struct SecondaryAttempt {
    pub result: Result<SecondaryEvaluation, SecondaryFailure>,
    pub calls: usize,
}
impl SecondaryWorker {
    pub fn new(
        factory: Arc<dyn SecondaryJudgeFactory>,
        policy: SecondaryPolicy,
    ) -> Result<Self, EvaluationError> {
        if policy.max_calls == 0
            || policy.max_request_bytes == 0
            || policy.max_generated_tokens == 0
            || policy.timeout.is_zero()
            || policy.timeout > Duration::from_secs(3600)
            || policy.queue_capacity == 0
            || policy.queue_capacity > 1024
        {
            return Err(EvaluationError::InvalidConfig(
                "invalid secondary budget/queue limits".into(),
            ));
        }
        let artifacts = factory.artifacts();
        if artifacts.is_empty() || artifacts.iter().any(|a| a.id.trim().is_empty()) {
            return Err(EvaluationError::InvalidConfig(
                "secondary artifact identity required".into(),
            ));
        }
        let (sender, receiver) = mpsc::sync_channel::<Work>(policy.queue_capacity);
        let calls = Arc::new(AtomicUsize::new(0));
        let worker_calls = calls.clone();
        let max_calls = policy.max_calls;
        std::thread::Builder::new()
            .name("kzn-secondary".into())
            .spawn(move || {
                // Cache successful or failed initialization; no load/retry storm.
                let mut judge: Option<Result<Box<dyn SecondaryJudge>, SecondaryFailure>> = None;
                while let Ok(work) = receiver.recv() {
                    let result = catch_unwind(AssertUnwindSafe(|| {
                        if Instant::now() >= work.request.deadline
                            || work.state.load(Ordering::SeqCst) == 2
                        {
                            return Err(SecondaryFailure::Timeout);
                        }
                        if worker_calls.load(Ordering::SeqCst) >= max_calls {
                            return Err(SecondaryFailure::BudgetExceeded);
                        }
                        let loaded = judge.get_or_insert_with(|| {
                            catch_unwind(AssertUnwindSafe(|| factory.load(work.request.deadline)))
                                .unwrap_or(Err(SecondaryFailure::BackendError))
                        });
                        let backend = loaded.as_mut().map_err(|failure| *failure)?;
                        if Instant::now() >= work.request.deadline
                            || work
                                .state
                                .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
                                .is_err()
                        {
                            return Err(SecondaryFailure::Timeout);
                        }
                        worker_calls.fetch_add(1, Ordering::SeqCst);
                        let output = backend.judge(&work.request)?;
                        if Instant::now() >= work.request.deadline {
                            return Err(SecondaryFailure::Timeout);
                        }
                        output.validate(&work.request)?;
                        if Instant::now() >= work.request.deadline {
                            return Err(SecondaryFailure::Timeout);
                        }
                        Ok(output)
                    }))
                    .unwrap_or_else(|_| {
                        // A panicked backend may be corrupted. Disable it until a new worker is supplied.
                        judge = Some(Err(SecondaryFailure::BackendError));
                        Err(SecondaryFailure::BackendError)
                    });
                    let _ = work.response.send(result);
                }
            })
            .map_err(|_| EvaluationError::Backend {
                backend: "secondary_worker".into(),
                message: "failed to spawn worker".into(),
            })?;
        Ok(Self {
            sender,
            policy,
            calls,
            artifacts,
        })
    }
    pub fn total_calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
    pub fn policy(&self) -> &SecondaryPolicy {
        &self.policy
    }
    pub(crate) fn artifacts(&self) -> Vec<ArtifactIdentity> {
        self.artifacts.clone()
    }
    pub(crate) fn run(&self, mut request: SecondaryRequest) -> SecondaryAttempt {
        if request
            .text
            .len()
            .saturating_add(request.reference.as_ref().map_or(0, String::len))
            > self.policy.max_request_bytes
            || self.total_calls() >= self.policy.max_calls
        {
            return SecondaryAttempt {
                result: Err(SecondaryFailure::BudgetExceeded),
                calls: 0,
            };
        }
        request.deadline = Instant::now() + self.policy.timeout;
        request.max_generated_tokens = self.policy.max_generated_tokens;
        let (response, receiver) = mpsc::sync_channel(1);
        let state = Arc::new(AtomicU8::new(0));
        let deadline = request.deadline;
        let work = Work {
            request,
            state: state.clone(),
            response,
        };
        let result = match self.sender.try_send(work) {
            Ok(()) => match receiver
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            {
                Ok(result) => result,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    let _ = state.compare_exchange(0, 2, Ordering::SeqCst, Ordering::SeqCst);
                    Err(SecondaryFailure::Timeout)
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => Err(SecondaryFailure::BackendError),
            },
            Err(mpsc::TrySendError::Full(_)) => Err(SecondaryFailure::BudgetExceeded),
            Err(mpsc::TrySendError::Disconnected(_)) => Err(SecondaryFailure::BackendError),
        };
        let result = if result.is_ok() && Instant::now() >= deadline {
            Err(SecondaryFailure::Timeout)
        } else {
            result
        };
        SecondaryAttempt {
            result,
            calls: usize::from(state.load(Ordering::SeqCst) == 1),
        }
    }
}
impl EvaluationEngine {
    pub fn with_secondary_worker(mut self, worker: Arc<SecondaryWorker>) -> Self {
        self.secondary = Some(worker);
        self
    }
    pub(crate) fn apply_secondary(&self, input: &TextInput<'_>, report: &mut EvaluationReport) {
        if report.routing.secondary_needed != Some(true) {
            return;
        }
        let Some(worker) = &self.secondary else {
            return;
        };
        if self.config.semantic_scope == ScoreScope::Reference
            && input.reference.is_none_or(|r| r.trim().is_empty())
        {
            report.routing.status = RoutingStatus::ContextMissing;
            report
                .routing
                .reasons
                .push("secondary_reference_context_required".into());
            report
                .limitations
                .retain(|s| s != "secondary_judge_not_configured");
            return;
        }
        let mut dimensions = Vec::new();
        if report.scores.naturalness.status != ScoreStatus::NotApplicable
            && (report.verdict == Verdict::Suspicious
                || self
                    .config
                    .required_dimensions
                    .contains(&Dimension::Naturalness)
                    && report.scores.naturalness.status != ScoreStatus::Evaluated)
        {
            dimensions.push(Dimension::Naturalness);
        }
        if self
            .config
            .required_dimensions
            .contains(&Dimension::SemanticConsistency)
            || input.reference.is_some_and(|r| !r.trim().is_empty())
        {
            dimensions.push(Dimension::SemanticConsistency);
        }
        if dimensions.is_empty() {
            return;
        }
        let attempt = if input
            .text
            .len()
            .saturating_add(input.reference.map_or(0, str::len))
            > worker.policy.max_request_bytes
            || worker.total_calls() >= worker.policy.max_calls
        {
            SecondaryAttempt {
                result: Err(SecondaryFailure::BudgetExceeded),
                calls: 0,
            }
        } else {
            let request = SecondaryRequest {
                text: input.text.into(),
                reference: input.reference.map(str::to_owned),
                profile_id: self.config.profile_id.clone(),
                primary_issues: report.issues.clone(),
                dimensions: dimensions.clone(),
                semantic_scope: self.config.semantic_scope,
                deadline: Instant::now(),
                max_generated_tokens: 0,
            };
            worker.run(request)
        };
        report.metrics.slm_calls = attempt.calls;
        report.provenance.extend(worker.artifacts());
        report
            .limitations
            .retain(|s| s != "secondary_judge_not_configured");
        report
            .limitations
            .push("secondary_model_scores_not_calibrated".into());
        match attempt.result {
            Ok(output) => {
                report.routing.status = RoutingStatus::Completed;
                for (dimension, value, threshold) in [
                    (
                        Dimension::Naturalness,
                        output.naturalness,
                        worker.policy.naturalness_threshold,
                    ),
                    (
                        Dimension::SemanticConsistency,
                        output.semantic_consistency,
                        worker.policy.semantic_threshold,
                    ),
                ] {
                    let Some(value) = value else {
                        continue;
                    };
                    let score = match dimension {
                        Dimension::Naturalness => &mut report.scores.naturalness,
                        Dimension::SemanticConsistency => &mut report.scores.semantic_consistency,
                        Dimension::Validity => unreachable!(),
                    };
                    *score = DimensionScore {
                        value: Some(value),
                        status: ScoreStatus::Evaluated,
                        method: Some(ScoreMethod::Model),
                        scope: score.scope,
                        calibration_id: None,
                        confidence: None,
                    };
                    if value.value() < threshold.value() {
                        report.issues.push(Issue {
                            code: match dimension {
                                Dimension::Naturalness => "secondary_naturalness_low",
                                _ => "secondary_semantic_consistency_low",
                            }
                            .into(),
                            severity: Severity::Warning,
                            span: ByteSpan::whole(input.text),
                            stage: DetectionStage::Secondary,
                            evidence: serde_json::json!({"score":value,"threshold":threshold}),
                            explanation:
                                "二次モデルの暫定閾値を下回りました。校正済確率ではありません。"
                                    .into(),
                        });
                    }
                }
                report.issues.extend(output.issues);
                report.verdict = if report.issues.iter().any(|i| i.severity == Severity::Error) { Verdict::Invalid }
                    else if report.issues.iter().any(|i| i.severity == Severity::Warning) { Verdict::Suspicious }
                    else if report.required_dimensions.iter().any(|d| match d {
                        Dimension::Validity => report.scores.validity.status,
                        Dimension::Naturalness => report.scores.naturalness.status,
                        Dimension::SemanticConsistency => report.scores.semantic_consistency.status,
                    } != ScoreStatus::Evaluated) { Verdict::Undetermined }
                    else { Verdict::Acceptable };
                if report
                    .issues
                    .iter()
                    .any(|i| i.stage == DetectionStage::Primary && i.severity == Severity::Warning)
                {
                    report
                        .limitations
                        .push("primary_warnings_retained_after_secondary".into());
                }
            }
            Err(failure) => {
                report.routing.status = match failure {
                    SecondaryFailure::BudgetExceeded => RoutingStatus::BudgetExceeded,
                    SecondaryFailure::Timeout => RoutingStatus::Timeout,
                    SecondaryFailure::BackendError => RoutingStatus::BackendError,
                    SecondaryFailure::InvalidOutput => RoutingStatus::InvalidOutput,
                };
                report.verdict = Verdict::Undetermined;
                report
                    .routing
                    .reasons
                    .push(format!("secondary_{failure:?}").to_lowercase());
                for dimension in dimensions {
                    let score = match dimension {
                        Dimension::Naturalness => &mut report.scores.naturalness,
                        Dimension::SemanticConsistency => &mut report.scores.semantic_consistency,
                        Dimension::Validity => unreachable!(),
                    };
                    *score = DimensionScore::unassessed(score.scope, ScoreStatus::Failed);
                }
                report
                    .limitations
                    .push("secondary_failed_or_skipped_primary_evidence_retained".into());
            }
        }
        for row in &mut report.coverage {
            let score = match row.dimension {
                Dimension::Validity => &report.scores.validity,
                Dimension::Naturalness => &report.scores.naturalness,
                Dimension::SemanticConsistency => &report.scores.semantic_consistency,
            };
            let evaluated = score.status == ScoreStatus::Evaluated;
            row.evaluated_spans = if evaluated {
                vec![ByteSpan::whole(input.text)]
            } else {
                vec![]
            };
            row.unassessed_spans = if evaluated {
                vec![]
            } else {
                vec![ByteSpan::whole(input.text)]
            };
        }
    }
}
