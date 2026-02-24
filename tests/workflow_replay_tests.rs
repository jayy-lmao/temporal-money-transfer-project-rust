use money_transfer_project_template_rust::{
    activity::Activities,
    shared::PaymentDetails,
    workflow::MoneyTransferWorkflow,
};
use rust_decimal::Decimal;
use temporal_test_harness::TestWorkflowEnvironment;

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
