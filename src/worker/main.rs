use std::str::FromStr;
use temporalio_client::{Client, ClientOptions, Connection, ConnectionOptions};
use temporalio_sdk::{Worker, WorkerOptions};
use temporalio_sdk_core::{CoreRuntime, RuntimeOptions, Url};

use money_transfer_project_template_rust::{
    activity::Activities, shared::MONEY_TRANSFER_TASK_QUEUE_NAME, workflow::MoneyTransferWorkflow,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = CoreRuntime::new_assume_tokio(RuntimeOptions::builder().build()?)?;

    let connection = Connection::connect(
        ConnectionOptions::new(Url::from_str("http://localhost:7233")?).build(),
    )
    .await?;

    let client = Client::new(connection, ClientOptions::new("default").build())?;

    let worker_options = WorkerOptions::new(MONEY_TRANSFER_TASK_QUEUE_NAME)
        .register_activities(Activities)
        .register_workflow::<MoneyTransferWorkflow>()
        .build();

    Worker::new(&runtime, client, worker_options)?.run().await?;
    Ok(())
}
