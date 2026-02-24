use temporalio_common::protos::temporal::api::{
    common::v1::Payloads,
    failure::v1::Failure,
};

/// The outcome of running a workflow in the test harness.
///
/// `Ok(Some(payloads))` — workflow completed successfully with a result payload.
/// `Ok(None)` — workflow completed successfully with no result.
/// `Err(failure)` — workflow failed (either via explicit FailWorkflowExecution or WFT failure).
pub(crate) type WorkflowTestResult = Result<Option<Payloads>, WorkflowFailure>;

/// Describes a workflow failure captured from the mock worker.
#[derive(Debug, Clone)]
pub struct WorkflowFailure {
    /// Human-readable failure message.
    pub message: String,
    /// The raw proto `Failure` if one was captured.
    pub failure: Option<Failure>,
    /// `true` when the failure came from a WFT failure (e.g. activity error propagated via `?`)
    /// rather than an explicit `FailWorkflowExecution` command.
    pub is_wft_failure: bool,
}

impl std::fmt::Display for WorkflowFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WorkflowFailure: {}", self.message)
    }
}

impl std::error::Error for WorkflowFailure {}

/// Harness-level errors (not workflow-level failures).
#[derive(Debug, thiserror::Error)]
pub enum TestHarnessError {
    #[error("workflow produced no result (neither completion nor failure was captured)")]
    NoResult,

    #[error("worker returned an error: {0}")]
    WorkerError(String),
}

/// Errors returned by [`TestWorkflowEnvironment::workflow_result`].
#[derive(Debug, thiserror::Error)]
pub enum WorkflowResultError {
    #[error("workflow has not been executed yet")]
    NotExecuted,
    #[error("workflow completed with error: {0}")]
    WorkflowFailed(WorkflowFailure),
    #[error("workflow produced no result payload")]
    NoResult,
    #[error("failed to deserialize result: {0}")]
    DeserializeError(String),
}
