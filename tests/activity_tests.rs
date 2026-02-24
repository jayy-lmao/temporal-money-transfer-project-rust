use money_transfer_project_template_rust::banking_client::BankingService;
use rust_decimal::Decimal;

fn bank() -> BankingService {
    BankingService {
        hostname: "test-bank.example.com".to_string(),
    }
}

#[test]
fn test_withdraw_success() {
    let bank = bank();
    let result = bank.withdraw("85-150".to_string(), Decimal::from(500), "ref-1".to_string());
    assert!(result.is_ok());
    let confirmation = result.unwrap();
    assert!(confirmation.starts_with("W"));
}

#[test]
fn test_withdraw_insufficient_funds() {
    let bank = bank();
    let result = bank.withdraw("85-150".to_string(), Decimal::from(5000), "ref-2".to_string());
    assert!(result.is_err());
    let err = result.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("insufficient funds"),
        "Expected 'insufficient funds' error, got: {msg}"
    );
}

#[test]
fn test_withdraw_invalid_account() {
    let bank = bank();
    let result = bank.withdraw("99-999".to_string(), Decimal::from(100), "ref-3".to_string());
    assert!(result.is_err());
    let err = result.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("no account found"),
        "Expected 'no account found' error, got: {msg}"
    );
}

#[test]
fn test_deposit_success() {
    let bank = bank();
    let result = bank.deposit("85-150".to_string(), Decimal::from(300), "ref-4".to_string());
    assert!(result.is_ok());
    let confirmation = result.unwrap();
    assert!(confirmation.starts_with("D"));
}

#[test]
fn test_deposit_that_fails_always_errors() {
    let bank = bank();
    let result =
        bank.deposit_that_fails("85-150".to_string(), Decimal::from(300), "ref-5".to_string());
    assert!(result.is_err());
    let err = result.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("no account found"),
        "Expected 'no account found' error, got: {msg}"
    );
}
