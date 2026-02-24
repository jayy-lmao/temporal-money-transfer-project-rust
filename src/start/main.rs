use rust_decimal::Decimal;
use std::str::FromStr;
use temporalio_client::{
    Client, ClientOptions, Connection, ConnectionOptions, WorkflowStartOptions,
};
use temporalio_sdk_core::{CoreRuntime, RuntimeOptions, Url};
use uuid::Uuid;

use temporal_rs_tutorial::shared::{MONEY_TRANSFER_TASK_QUEUE_NAME, PaymentDetails};
use temporal_rs_tutorial::workflow::MoneyTransferWorkflow;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _runtime = CoreRuntime::new_assume_tokio(RuntimeOptions::builder().build()?)?;
    let connection = Connection::connect(
        ConnectionOptions::new(Url::from_str("http://localhost:7233")?).build(),
    )
    .await?;
    let client = Client::new(connection, ClientOptions::new("default").build())?;

    let payment = PaymentDetails {
        amount: Decimal::new(400, 2), // 4.00
        source_account: "85-150".to_string(),
        target_account: "43-812".to_string(),
        reference_id: Uuid::new_v4().to_string(),
    };

    let workflow_id = format!("pay-invoice-{}", &payment.reference_id);
    let options =
        WorkflowStartOptions::new(MONEY_TRANSFER_TASK_QUEUE_NAME, workflow_id.clone()).build();

    let handle = client
        .start_workflow(MoneyTransferWorkflow, payment, options)
        .await?;

    println!(
        "Started workflow {workflow_id}, run_id: {}",
        handle.run_id().unwrap_or("<unknown>")
    );
    Ok(())
}
