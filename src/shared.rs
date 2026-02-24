use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

pub const MONEY_TRANSFER_TASK_QUEUE_NAME: &str = "TRANSFER_MONEY_TASK_QUEUE";

#[derive(Deserialize, Serialize)]
pub struct PaymentDetails {
    pub amount: Decimal,
    pub source_account: String,
    pub target_account: String,
    pub reference_id: String,
}
