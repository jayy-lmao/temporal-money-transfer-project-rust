use std::process::Stdio;
use std::str::FromStr;

use money_transfer_project_template_rust::{
    activity::Activities,
    shared::{PaymentDetails, MONEY_TRANSFER_TASK_QUEUE_NAME},
    workflow::MoneyTransferWorkflow,
};
use rust_decimal::Decimal;
use temporal_test_harness::TestWorkflowEnvironment;
use temporalio_client::{
    Client, ClientOptions, Connection, ConnectionOptions, WorkflowGetResultOptions,
    WorkflowStartOptions,
};
use temporalio_sdk::{Worker, WorkerOptions};
use temporalio_sdk_core::{
    ephemeral_server::{default_cached_download, TemporalDevServerConfig},
    CoreRuntime, RuntimeOptions, Url,
};

/// Build a PaymentDetails input for tests.
fn test_payment_details() -> PaymentDetails {
    PaymentDetails {
        amount: Decimal::from(400),
        source_account: "85-150".to_string(),
        target_account: "43-812".to_string(),
        reference_id: "test-ref-001".to_string(),
    }
}

#[tokio::test]
async fn test_money_transfer_happy_path() {
    let mut env = TestWorkflowEnvironment::new();
    env.register_activities(Activities);
    env.on_activity("Activities::withdraw").returns("W1234567890");
    env.on_activity("Activities::deposit").returns("D0987654321");

    env.execute_workflow::<MoneyTransferWorkflow>(test_payment_details())
        .await
        .expect("harness should not error");

    assert!(env.is_workflow_completed());
    assert!(env.workflow_error().is_none());
    let result: String = env.workflow_result().unwrap();
    assert!(
        result.contains("W1234567890"),
        "Expected withdraw ID in output: {result}"
    );
    assert!(
        result.contains("D0987654321"),
        "Expected deposit ID in output: {result}"
    );
}

#[tokio::test]
async fn test_money_transfer_deposit_fails() {
    let mut env = TestWorkflowEnvironment::new();
    env.register_activities(Activities);
    env.on_activity("Activities::withdraw").returns("W1234567890");
    env.on_activity("Activities::deposit").returns_err("deposit failed");

    env.execute_workflow::<MoneyTransferWorkflow>(test_payment_details())
        .await
        .expect("harness should not error");

    assert!(env.is_workflow_completed());
    assert!(env.workflow_error().is_some());
}

#[tokio::test]
async fn test_money_transfer_withdraw_fails() {
    let mut env = TestWorkflowEnvironment::new();
    env.register_activities(Activities);
    env.on_activity("Activities::withdraw").returns_err("withdraw failed");

    env.execute_workflow::<MoneyTransferWorkflow>(test_payment_details())
        .await
        .expect("harness should not error");

    assert!(env.is_workflow_completed());
    assert!(env.workflow_error().is_some());
}

#[tokio::test(flavor = "current_thread")]
async fn test_money_transfer_with_dev_server() {
    // 1. Start ephemeral dev server
    let config = TemporalDevServerConfig::builder()
        .exe(default_cached_download())
        .log(("pretty".to_string(), "error".to_string()))
        .build();

    let mut server = config
        .start_server_with_output(Stdio::null(), Stdio::null())
        .await
        .expect("Failed to start ephemeral dev server");

    let server_addr = format!("http://{}", server.target);

    // 2. Create runtime and worker connection
    let runtime =
        CoreRuntime::new_assume_tokio(RuntimeOptions::builder().build().unwrap()).unwrap();

    let worker_connection = Connection::connect(
        ConnectionOptions::new(Url::from_str(&server_addr).unwrap())
            .identity("integration-test-worker".to_string())
            .build(),
    )
    .await
    .expect("Failed to connect worker to dev server");

    let worker_client =
        Client::new(worker_connection, ClientOptions::new("default").build()).unwrap();

    // 3. Build worker
    let worker_options = WorkerOptions::new(MONEY_TRANSFER_TASK_QUEUE_NAME)
        .register_activities(Activities)
        .register_workflow::<MoneyTransferWorkflow>()
        .build();

    let mut worker = Worker::new(&runtime, worker_client, worker_options)
        .expect("Failed to create worker");
    let shutdown_handle = worker.shutdown_handle();

    // 4. Create a second client connection for starting workflows
    let starter_connection = Connection::connect(
        ConnectionOptions::new(Url::from_str(&server_addr).unwrap())
            .identity("integration-test-starter".to_string())
            .build(),
    )
    .await
    .expect("Failed to connect starter client to dev server");

    let starter_client =
        Client::new(starter_connection, ClientOptions::new("default").build()).unwrap();

    // 5. Run worker and workflow execution concurrently.
    //    Worker is !Send so we cannot tokio::spawn it; instead we use tokio::select!
    //    on a current_thread runtime so both futures run on the same thread.
    let result: String = tokio::select! {
        worker_result = worker.run() => {
            panic!("Worker exited unexpectedly: {:?}", worker_result);
        }
        workflow_result = async {
            let payment = PaymentDetails {
                amount: Decimal::new(400, 2), // 4.00
                source_account: "85-150".to_string(),
                target_account: "43-812".to_string(),
                reference_id: uuid::Uuid::new_v4().to_string(),
            };

            let workflow_id = format!("integration-test-{}", uuid::Uuid::new_v4());
            let options = WorkflowStartOptions::new(
                MONEY_TRANSFER_TASK_QUEUE_NAME,
                workflow_id,
            )
            .build();

            let handle = starter_client
                .start_workflow(MoneyTransferWorkflow, payment, options)
                .await
                .expect("Failed to start workflow");

            handle
                .get_result(WorkflowGetResultOptions::default())
                .await
                .expect("Workflow execution failed")
        } => {
            // Shut down the worker now that we have the result
            shutdown_handle();
            workflow_result
        }
    };

    // 6. Assert on the result
    assert!(
        result.contains("Transfer complete"),
        "Expected 'Transfer complete' in result: {result}"
    );
    assert!(
        result.contains("transaction IDs:"),
        "Expected 'transaction IDs:' in result: {result}"
    );

    // 7. Shutdown server
    server.shutdown().await.expect("Failed to shutdown dev server");
}
