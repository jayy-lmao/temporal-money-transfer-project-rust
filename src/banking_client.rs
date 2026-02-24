use rand::Rng;
use rust_decimal::Decimal;
use std::sync::LazyLock;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
#[error(
    "insufficient funds: current balance is {current_balance}, attempted withdrawal of {attempted_withdrawal}"
)]
pub struct InsufficientFundsError {
    pub current_balance: Decimal,
    pub attempted_withdrawal: Decimal,
}

#[derive(Debug, thiserror::Error)]
#[error("no account found with number {account_number}")]
pub struct InvalidAccountError {
    pub account_number: String,
}

#[derive(Debug, thiserror::Error)]
pub enum BankingError {
    #[error(transparent)]
    InsufficientFunds(#[from] InsufficientFundsError),
    #[error(transparent)]
    InvalidAccount(#[from] InvalidAccountError),
}

// ---------------------------------------------------------------------------
// Account & Bank
// ---------------------------------------------------------------------------

pub struct Account {
    pub account_number: String,
    pub balance: Decimal,
}

pub struct Bank {
    accounts: Vec<Account>,
}

impl Bank {
    pub fn find_account(&self, account_number: &str) -> Result<&Account, InvalidAccountError> {
        self.accounts
            .iter()
            .find(|a| a.account_number == account_number)
            .ok_or_else(|| InvalidAccountError {
                account_number: account_number.to_string(),
            })
    }
}

static MOCK_BANK: LazyLock<Bank> = LazyLock::new(|| Bank {
    accounts: vec![
        Account {
            account_number: "85-150".to_string(),
            balance: Decimal::from(2000),
        },
        Account {
            account_number: "43-812".to_string(),
            balance: Decimal::from(0),
        },
    ],
});

// ---------------------------------------------------------------------------
// BankingService
// ---------------------------------------------------------------------------

pub struct BankingService {
    pub hostname: String,
}

impl BankingService {
    pub fn withdraw(
        &self,
        account_number: String,
        amount: Decimal,
        reference_id: String,
    ) -> Result<String, BankingError> {
        let account = MOCK_BANK.find_account(&account_number)?;
        if account.balance < amount {
            return Err(InsufficientFundsError {
                current_balance: account.balance,
                attempted_withdrawal: amount,
            }
            .into());
        }
        let confirmation = generate_transaction_id("W", 10);
        println!(
            "Withdrawal of {} from account {} accepted. Confirmation: {}. ReferenceId: {}",
            amount, account_number, confirmation, reference_id
        );
        Ok(confirmation)
    }

    pub fn deposit(
        &self,
        account_number: String,
        amount: Decimal,
        reference_id: String,
    ) -> Result<String, BankingError> {
        let _ = MOCK_BANK.find_account(&account_number)?;
        let confirmation = generate_transaction_id("D", 10);
        println!(
            "Deposit of {} to account {} accepted. Confirmation: {}. ReferenceId: {}",
            amount, account_number, confirmation, reference_id
        );
        Ok(confirmation)
    }

    pub fn deposit_that_fails(
        &self,
        account_number: &str,
        _amount: i64,
        _reference_id: &str,
    ) -> Result<String, BankingError> {
        let _ = MOCK_BANK.find_account(account_number)?;
        let _confirmation = generate_transaction_id("D", 10);
        Err(InvalidAccountError {
            account_number: account_number.to_string(),
        }
        .into())
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn generate_transaction_id(prefix: &str, length: usize) -> String {
    let mut rng = rand::rng();
    let digits: String = (0..length)
        .map(|_| rng.random_range(0..10).to_string())
        .collect();
    format!("{}{}", prefix, digits)
}
