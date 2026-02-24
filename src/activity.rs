use temporalio_macros::{activities, activity};
use temporalio_sdk::activities::{ActivityContext, ActivityError};

use crate::{banking_client::BankingService, shared::PaymentDetails};

#[activity]
#[derive(Default)]
pub struct Activities;

#[activities]
impl Activities {
    #[activity]
    pub async fn withdraw(
        _ctx: ActivityContext,
        data: PaymentDetails,
    ) -> Result<String, ActivityError> {
        println!(
            "Withdrawing {} from account {}.\n",
            data.amount, data.source_account
        );

        let reference_id = format!("{}-withdrawal", data.reference_id);
        let bank = BankingService {
            hostname: "bank-api.example.com".to_string(),
        };

        let confirmation = bank.withdraw(data.source_account, data.amount, reference_id)?;

        Ok(confirmation)
    }

    #[activity]
    pub async fn deposit(
        _ctx: ActivityContext,
        data: PaymentDetails,
    ) -> Result<String, ActivityError> {
        println!(
            "Despositing {} into account {}.\n",
            data.amount, data.target_account
        );

        let reference_id = format!("{}-deposit", data.reference_id);
        let bank = BankingService {
            hostname: "bank-api.example.com".to_string(),
        };

        let confirmation = bank.deposit(data.source_account, data.amount, reference_id)?;

        Ok(confirmation)
    }

    #[activity]
    pub async fn refund(
        _ctx: ActivityContext,
        data: PaymentDetails,
    ) -> Result<String, ActivityError> {
        println!(
            "Refunding {} back into account {}.\n",
            data.amount, data.source_account
        );

        let reference_id = format!("{}-refund", data.reference_id);
        let bank = BankingService {
            hostname: "bank-api.example.com".to_string(),
        };

        let confirmation = bank.deposit(data.target_account, data.amount, reference_id)?;

        Ok(confirmation)
    }
}
