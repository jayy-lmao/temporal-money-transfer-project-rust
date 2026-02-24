use prost_wkt_types::Duration;
use temporalio_common::protos::temporal::api::common::v1::RetryPolicy;
use temporalio_macros::{workflow, workflow_methods};
use temporalio_sdk::{ActivityOptions, WorkflowContext, WorkflowResult};

use crate::{activity::Activities, shared::PaymentDetails};

#[workflow]
#[derive(Default)]
pub struct MoneyTransferWorkflow;

#[workflow_methods]
impl MoneyTransferWorkflow {
    #[run]
    pub async fn run(
        ctx: &mut WorkflowContext<Self>,
        input: PaymentDetails,
    ) -> WorkflowResult<String> {
        let retry_policy = RetryPolicy {
            initial_interval: Some(Duration {
                seconds: 1,
                nanos: 0,
            }),
            backoff_coefficient: 2.0,
            maximum_interval: Some(Duration {
                seconds: 5,
                nanos: 0,
            }),
            maximum_attempts: 5,
            non_retryable_error_types: vec![
                "InvalidAccountError".to_string(),
                "InsufficentFundsError".to_string(),
            ],
        };
        let options = ActivityOptions {
            start_to_close_timeout: Some(std::time::Duration::from_secs_f64(60.)),
            retry_policy: Some(retry_policy),
            ..Default::default()
        };

        let res = ctx
            .start_activity(Activities::withdraw, input, options)
            .await?;

        Ok(res)
    }
}
