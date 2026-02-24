use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::Serialize;
use tokio::sync::Notify;
use temporalio_common::{
    data_converters::DataConverter,
    protos::{
        coresdk::{AsJsonPayloadExt, FromJsonPayloadExt},
        temporal::api::{
            command::v1::command,
            common::v1::Payloads,
            failure::v1::Failure,
        },
    },
};
use temporalio_sdk::{
    Worker as SdkWorker,
    activities::ActivityImplementer,
    workflows::WorkflowImplementer,
};
use temporalio_sdk_core::test_help::{MockPollCfg, build_mock_pollers, mock_worker};

use crate::error::{TestHarnessError, WorkflowFailure, WorkflowResultError, WorkflowTestResult};
use crate::history::{ActivityMock, build_history};

/// Captured result from the mock worker — populated by callbacks.
#[derive(Default)]
struct CapturedResult {
    /// Set when a `CompleteWorkflowExecution` command is observed.
    success: Option<Option<Payloads>>,
    /// Set when a `FailWorkflowExecution` command is observed.
    command_failure: Option<WorkflowFailure>,
    /// Set when a WFT failure is observed (activity error propagated via `?`).
    wft_failure: Option<WorkflowFailure>,
}

// Type-erased closures for registering workflows/activities on an SdkWorker.
type WorkflowRegistrar = Box<dyn FnOnce(&mut SdkWorker) + Send>;
type ActivityRegistrar = Box<dyn FnOnce(&mut SdkWorker) + Send>;

/// A mutable test environment that mirrors Go's `TestWorkflowEnvironment`.
///
/// # Example
/// ```ignore
/// let mut env = TestWorkflowEnvironment::new();
/// env.register_activities(Activities);
/// env.on_activity("Activities::withdraw").returns("W1234");
/// env.on_activity("Activities::deposit").returns("D5678");
///
/// env.execute_workflow::<MoneyTransferWorkflow>(payment_details).await;
///
/// assert!(env.is_workflow_completed());
/// assert!(env.workflow_error().is_none());
/// let result: String = env.workflow_result().unwrap();
/// ```
pub struct TestWorkflowEnvironment {
    activity_registrar: Option<ActivityRegistrar>,
    activity_mocks: Vec<(String, ActivityMock)>,
    timeout: Duration,
    // Post-execution state
    completed: bool,
    result: Option<WorkflowTestResult>,
}

impl TestWorkflowEnvironment {
    /// Create a new test environment with default settings (15s timeout).
    pub fn new() -> Self {
        Self {
            activity_registrar: None,
            activity_mocks: Vec::new(),
            timeout: Duration::from_secs(15),
            completed: false,
            result: None,
        }
    }

    /// Register an activity implementation to be available during the test.
    pub fn register_activities<A: ActivityImplementer + Send + 'static>(&mut self, instance: A) {
        self.activity_registrar = Some(Box::new(move |worker: &mut SdkWorker| {
            worker.register_activities(instance);
        }));
    }

    /// Begin mocking an activity by name. Call `.returns(val)` or `.returns_err(msg)`
    /// on the returned handle to complete the mock.
    pub fn on_activity(&mut self, name: &str) -> ActivityMockCall<'_> {
        ActivityMockCall {
            activity_mocks: &mut self.activity_mocks,
            name: name.to_string(),
        }
    }

    /// Set the timeout for the worker run. Defaults to 15 seconds.
    pub fn set_test_timeout(&mut self, duration: Duration) {
        self.timeout = duration;
    }

    /// Execute the workflow, storing the result internally.
    ///
    /// After this returns, use [`is_workflow_completed`], [`workflow_error`], and
    /// [`workflow_result`] to inspect the outcome.
    pub async fn execute_workflow<W>(
        &mut self,
        input: impl Serialize,
    ) -> Result<(), TestHarnessError>
    where
        W: WorkflowImplementer + 'static,
        <W::Run as temporalio_common::WorkflowDefinition>::Input: Send,
    {
        let workflow_name = W::name().to_string();
        let workflow_registrar: WorkflowRegistrar = Box::new(|worker: &mut SdkWorker| {
            worker.register_workflow::<W>();
        });

        let payload = input
            .as_json_payload()
            .expect("input must be JSON-serializable");
        let input_payloads = Payloads {
            payloads: vec![payload],
        };

        let result = execute_internal(
            &workflow_name,
            workflow_registrar,
            self.activity_registrar.take(),
            input_payloads,
            &self.activity_mocks,
            self.timeout,
        )
        .await?;

        self.completed = true;
        self.result = Some(result);
        Ok(())
    }

    /// Returns `true` if the workflow has finished executing (success or failure).
    pub fn is_workflow_completed(&self) -> bool {
        self.completed
    }

    /// Returns the workflow failure, if any.
    pub fn workflow_error(&self) -> Option<&WorkflowFailure> {
        match &self.result {
            Some(Err(failure)) => Some(failure),
            _ => None,
        }
    }

    /// Deserialize and return the successful workflow result.
    pub fn workflow_result<T: DeserializeOwned>(&self) -> Result<T, WorkflowResultError> {
        let result = self.result.as_ref().ok_or(WorkflowResultError::NotExecuted)?;
        match result {
            Err(failure) => Err(WorkflowResultError::WorkflowFailed(failure.clone())),
            Ok(None) => Err(WorkflowResultError::NoResult),
            Ok(Some(payloads)) => {
                let payload = payloads
                    .payloads
                    .first()
                    .ok_or(WorkflowResultError::NoResult)?;
                T::from_json_payload(payload)
                    .map_err(|e| WorkflowResultError::DeserializeError(format!("{e}")))
            }
        }
    }
}

/// Fluent handle for mocking a single activity.
///
/// Created by [`TestWorkflowEnvironment::on_activity`]. The borrow is released
/// when `.returns()` or `.returns_err()` is called (both consume `self`).
pub struct ActivityMockCall<'a> {
    activity_mocks: &'a mut Vec<(String, ActivityMock)>,
    name: String,
}

impl ActivityMockCall<'_> {
    /// Mock this activity to succeed with the given JSON-serializable value.
    pub fn returns<T: Serialize>(self, value: T) {
        let payload = value
            .as_json_payload()
            .expect("activity result must be JSON-serializable");
        self.activity_mocks
            .push((self.name, ActivityMock::Success(payload)));
    }

    /// Mock this activity to fail with the given error message.
    pub fn returns_err(self, message: &str) {
        self.activity_mocks
            .push((self.name, ActivityMock::Failure(message.to_string())));
    }
}

/// Shared execution logic used by `TestWorkflowEnvironment`.
async fn execute_internal(
    workflow_name: &str,
    workflow_registrar: WorkflowRegistrar,
    activity_registrar: Option<ActivityRegistrar>,
    input_payloads: Payloads,
    activity_mocks: &[(String, ActivityMock)],
    timeout: Duration,
) -> Result<WorkflowTestResult, TestHarnessError> {
    // Build synthetic history
    let (t, has_failure) = build_history(workflow_name, input_payloads, activity_mocks);

    // Shared state for capturing results
    let captured = Arc::new(Mutex::new(CapturedResult::default()));
    // Notification to signal when a terminal result has been captured
    let done = Arc::new(Notify::new());

    // Configure mocks
    let mut mock_cfg = MockPollCfg::from_hist_builder(t);
    mock_cfg.using_rust_sdk = true;
    mock_cfg.make_poll_stream_interminable = true;

    if has_failure {
        mock_cfg.num_expected_fails = 1;

        // Capture WFT failures (activity error propagated via `?`)
        let captured_for_fail = Arc::clone(&captured);
        let done_for_fail = Arc::clone(&done);
        mock_cfg.expect_fail_wft_matcher =
            Box::new(move |_task_token, _cause, failure: &Option<Failure>| {
                let mut cap = captured_for_fail.lock().unwrap();
                let msg = failure
                    .as_ref()
                    .map(|f| f.message.clone())
                    .unwrap_or_else(|| "WFT failure (no message)".to_string());
                cap.wft_failure = Some(WorkflowFailure {
                    message: msg,
                    failure: failure.clone(),
                    is_wft_failure: true,
                });
                done_for_fail.notify_one();
                true // accept the failure
            });
    }

    // Capture successful completions and explicit failures
    let captured_for_completion = Arc::clone(&captured);
    let done_for_completion = Arc::clone(&done);
    mock_cfg.completion_mock_fn = Some(Box::new(move |completion| {
        let mut cap = captured_for_completion.lock().unwrap();
        for cmd in &completion.commands {
            if let Some(ref attrs) = cmd.attributes {
                match attrs {
                    command::Attributes::CompleteWorkflowExecutionCommandAttributes(
                        complete,
                    ) => {
                        cap.success = Some(complete.result.clone());
                        done_for_completion.notify_one();
                    }
                    command::Attributes::FailWorkflowExecutionCommandAttributes(fail) => {
                        let msg = fail
                            .failure
                            .as_ref()
                            .map(|f| f.message.clone())
                            .unwrap_or_else(|| "workflow failed (no message)".to_string());
                        cap.command_failure = Some(WorkflowFailure {
                            message: msg,
                            failure: fail.failure.clone(),
                            is_wft_failure: false,
                        });
                        done_for_completion.notify_one();
                    }
                    _ => {} // Ignore other commands (ScheduleActivity, etc.)
                }
            }
        }
        Ok(Default::default())
    }));

    // Build mock infrastructure
    let mh = build_mock_pollers(mock_cfg);
    let core_worker = mock_worker(mh);

    // Build SDK worker and register workflow + activities
    let mut worker = SdkWorker::new_from_core(Arc::new(core_worker), DataConverter::default());
    (workflow_registrar)(&mut worker);
    if let Some(activity_registrar) = activity_registrar {
        (activity_registrar)(&mut worker);
    }

    // Race the worker against the done notification.
    tokio::select! {
        result = worker.run() => {
            if let Err(e) = result {
                return Err(TestHarnessError::WorkerError(format!("{e:#}")));
            }
        }
        _ = done.notified() => {}
        _ = tokio::time::sleep(timeout) => {}
    }

    // Extract the captured result
    let cap = captured.lock().unwrap();
    if let Some(ref failure) = cap.command_failure {
        return Ok(Err(failure.clone()));
    }
    if let Some(ref failure) = cap.wft_failure {
        return Ok(Err(failure.clone()));
    }
    if let Some(ref payloads) = cap.success {
        return Ok(Ok(payloads.clone()));
    }

    Err(TestHarnessError::NoResult)
}
