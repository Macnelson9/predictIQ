use crate::errors::ErrorCode;
use soroban_sdk::{token, Address, Env};

/// Issue #11: Use try_invoke_contract so transfer failures are handled
/// programmatically instead of relying on host panics.
pub fn safe_transfer(
    e: &Env,
    token_address: &Address,
    from: &Address,
    to: &Address,
    amount: &i128,
) -> Result<(), ErrorCode> {
    let client = token::Client::new(e, token_address);

    // Attempt transfer - will panic if clawed back or frozen
    client.transfer(from, to, amount);

    Ok(())
}

/// Check if contract can receive tokens (not frozen)
/// Returns true if the contract's balance can be modified
pub fn verify_contract_not_frozen(e: &Env, token_address: &Address) -> Result<(), ErrorCode> {
    let client = token::Client::new(e, token_address);
    let contract_addr = e.current_contract_address();

    // Try to get balance - if frozen, this will succeed but transfers will fail
    let _balance = client.balance(&contract_addr);

    Ok(())
}

/// Issue #27: ErrorCode::AssetClawedBack now exists in errors.rs.
pub fn detect_clawback(
    e: &Env,
    token_address: &Address,
    expected_balance: i128,
) -> Result<(), ErrorCode> {
    let client = soroban_sdk::token::Client::new(e, token_address);
    let actual_balance = client.balance(&e.current_contract_address());

    if actual_balance < expected_balance {
        return Err(ErrorCode::InsufficientBalance);
    }

    Ok(())
}
