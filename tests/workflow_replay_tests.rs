use std::sync::Arc;

use money_transfer_project_template_rust::{
    activity::Activities,
    shared::PaymentDetails,
    workflow::MoneyTransferWorkflow,
};
use rust_decimal::Decimal;
use temporalio_common::{
    data_converters::DataConverter,
    protos::{
        coresdk::{AsJsonPayloadExt, IntoPayloadsExt},
        temporal::api::{
            common::v1::ActivityType,
            failure::v1::Failure,
            history::v1::{
                ActivityTaskCompletedEventAttributes, ActivityTaskFailedEventAttributes,
                ActivityTaskScheduledEventAttributes, ActivityTaskStartedEventAttributes,
                history_event::Attributes,
            },
        },
    },
};
use temporalio_sdk::Worker as SdkWorker;
use temporalio_sdk_core::test_help::{build_mock_pollers, mock_worker, MockPollCfg};

/// Build a PaymentDetails input for tests.
fn test_payment_details() -> PaymentDetails {
    PaymentDetails {
        amount: Decimal::from(400),
        source_account: "85-150".to_string(),
        target_account: "43-812".to_string(),
        reference_id: "test-ref-001".to_string(),
    }
}

/// Helper: schedule + start + complete an activity in the history builder.
fn add_successful_activity(
    t: &mut temporalio_sdk_core::replay::TestHistoryBuilder,
    activity_id: &str,
    activity_type_name: &str,
    result_payload: &str,
) {
    let scheduled_event_id = t.add(ActivityTaskScheduledEventAttributes {
        activity_id: activity_id.to_string(),
        activity_type: Some(ActivityType {
            name: activity_type_name.to_string(),
        }),
        ..Default::default()
    });
    let started_event_id = t.add(Attributes::ActivityTaskStartedEventAttributes(
        ActivityTaskStartedEventAttributes {
            scheduled_event_id,
            ..Default::default()
        },
    ));
    t.add(ActivityTaskCompletedEventAttributes {
        scheduled_event_id,
        started_event_id,
        result: vec![result_payload.as_json_payload().unwrap()].into_payloads(),
        ..Default::default()
    });
}

/// Helper: schedule + start + fail an activity in the history builder.
fn add_failed_activity(
    t: &mut temporalio_sdk_core::replay::TestHistoryBuilder,
    activity_id: &str,
    activity_type_name: &str,
    failure_message: &str,
) {
    let scheduled_event_id = t.add(ActivityTaskScheduledEventAttributes {
        activity_id: activity_id.to_string(),
        activity_type: Some(ActivityType {
            name: activity_type_name.to_string(),
        }),
        ..Default::default()
    });
    let started_event_id = t.add(Attributes::ActivityTaskStartedEventAttributes(
        ActivityTaskStartedEventAttributes {
            scheduled_event_id,
            ..Default::default()
        },
    ));
    t.add(Attributes::ActivityTaskFailedEventAttributes(
        ActivityTaskFailedEventAttributes {
            scheduled_event_id,
            started_event_id,
            failure: Some(Failure {
                message: failure_message.to_string(),
                ..Default::default()
            }),
            ..Default::default()
        },
    ));
}

/// Create an SDK worker wrapping a mock core worker, with our workflow and activities registered.
fn build_sdk_worker(core_worker: temporalio_sdk_core::Worker) -> SdkWorker {
    let mut worker =
        SdkWorker::new_from_core(Arc::new(core_worker), DataConverter::default());
    worker.register_workflow::<MoneyTransferWorkflow>();
    worker.register_activities(Activities);
    worker
}

#[tokio::test]
async fn test_money_transfer_happy_path() {
    use temporalio_sdk_core::replay::TestHistoryBuilder;
    use temporalio_common::protos::temporal::api::enums::v1::EventType;

    // Build the synthetic history for a successful money transfer
    let mut t = TestHistoryBuilder::default();
    t.add_by_type(EventType::WorkflowExecutionStarted);
    t.set_wf_type("MoneyTransferWorkflow");
    t.set_wf_input(
        vec![test_payment_details().as_json_payload().unwrap()]
            .into_payloads()
            .unwrap(),
    );

    // WFT 1: workflow starts, schedules withdraw activity
    t.add_full_wf_task();
    add_successful_activity(&mut t, "1", "Activities::withdraw", "W1234567890");

    // WFT 2: withdraw resolved, schedules deposit activity
    t.add_full_wf_task();
    add_successful_activity(&mut t, "2", "Activities::deposit", "D0987654321");

    // WFT 3: deposit resolved, workflow completes
    t.add_workflow_task_scheduled_and_started();

    // Build mock infrastructure
    let mut mock_cfg = MockPollCfg::from_hist_builder(t);
    mock_cfg.using_rust_sdk = true;
    mock_cfg.make_poll_stream_interminable = true;

    let mh = build_mock_pollers(mock_cfg);
    let core_worker = mock_worker(mh);

    let mut worker = build_sdk_worker(core_worker);

    // Run the worker with a timeout — the mock stream blocks forever after work is done
    let result = tokio::time::timeout(std::time::Duration::from_secs(10), worker.run()).await;

    // The worker will block on the interminable stream; timeout is expected.
    // What matters is that no panic or nondeterminism error occurred during replay.
    assert!(
        result.is_err(),
        "Expected timeout (worker blocks after replay completes), but got: {result:?}"
    );
}

#[tokio::test]
async fn test_money_transfer_deposit_fails() {
    use temporalio_sdk_core::replay::TestHistoryBuilder;
    use temporalio_common::protos::temporal::api::enums::v1::EventType;

    let mut t = TestHistoryBuilder::default();
    t.add_by_type(EventType::WorkflowExecutionStarted);
    t.set_wf_type("MoneyTransferWorkflow");
    t.set_wf_input(
        vec![test_payment_details().as_json_payload().unwrap()]
            .into_payloads()
            .unwrap(),
    );

    // WFT 1: workflow starts, schedules withdraw
    t.add_full_wf_task();
    add_successful_activity(&mut t, "1", "Activities::withdraw", "W1234567890");

    // WFT 2: withdraw resolved, schedules deposit
    t.add_full_wf_task();
    add_failed_activity(&mut t, "2", "Activities::deposit", "deposit failed");

    // WFT 3: deposit failure propagated — workflow should fail
    t.add_workflow_task_scheduled_and_started();

    let mut mock_cfg = MockPollCfg::from_hist_builder(t);
    mock_cfg.using_rust_sdk = true;
    mock_cfg.make_poll_stream_interminable = true;
    // The workflow propagates the activity failure via `?`, which causes the WFT to fail
    mock_cfg.num_expected_fails = 1;

    let mh = build_mock_pollers(mock_cfg);
    let core_worker = mock_worker(mh);

    let mut worker = build_sdk_worker(core_worker);

    let result = tokio::time::timeout(std::time::Duration::from_secs(10), worker.run()).await;

    // Timeout expected — worker blocks after replay completes
    assert!(
        result.is_err(),
        "Expected timeout, but got: {result:?}"
    );
}

#[tokio::test]
async fn test_money_transfer_withdraw_fails() {
    use temporalio_sdk_core::replay::TestHistoryBuilder;
    use temporalio_common::protos::temporal::api::enums::v1::EventType;

    let mut t = TestHistoryBuilder::default();
    t.add_by_type(EventType::WorkflowExecutionStarted);
    t.set_wf_type("MoneyTransferWorkflow");
    t.set_wf_input(
        vec![test_payment_details().as_json_payload().unwrap()]
            .into_payloads()
            .unwrap(),
    );

    // WFT 1: workflow starts, schedules withdraw — which fails immediately
    t.add_full_wf_task();
    add_failed_activity(&mut t, "1", "Activities::withdraw", "withdraw failed");

    // WFT 2: withdraw failure propagated — workflow should fail without scheduling deposit
    t.add_workflow_task_scheduled_and_started();

    let mut mock_cfg = MockPollCfg::from_hist_builder(t);
    mock_cfg.using_rust_sdk = true;
    mock_cfg.make_poll_stream_interminable = true;
    // The workflow propagates the activity failure via `?`
    mock_cfg.num_expected_fails = 1;

    let mh = build_mock_pollers(mock_cfg);
    let core_worker = mock_worker(mh);

    let mut worker = build_sdk_worker(core_worker);

    let result = tokio::time::timeout(std::time::Duration::from_secs(10), worker.run()).await;

    assert!(
        result.is_err(),
        "Expected timeout, but got: {result:?}"
    );
}
