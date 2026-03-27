use crate::errors::ErrorCode;
use crate::types::OracleConfig;
use soroban_sdk::{contracttype, symbol_short, Env};

const BPS_DENOMINATOR: u64 = 10_000;

/// Issue #9: Key now includes oracle_id to support multi-oracle aggregation.
#[contracttype]
pub enum OracleData {
    Result(u64, u32),     // (market_id, oracle_id) -> outcome
    LastUpdate(u64, u32), // (market_id, oracle_id) -> timestamp
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythPrice {
    pub price: i64,
    pub conf: u64,
    pub expo: i32,
    /// Issue #49: stored as u64 to match ledger timestamp type.
    pub publish_time: u64,
}

/// Issue #25: In production replace this stub with a real Pyth cross-contract call.
/// The function signature is kept so callers compile; the implementation
/// returns OracleFailure until a real integration is wired in.
pub fn fetch_pyth_price(_e: &Env, _config: &OracleConfig) -> Result<PythPrice, ErrorCode> {
    Err(ErrorCode::OracleFailure)
}

pub fn cast_external_timestamp(timestamp: i64) -> Result<u64, ErrorCode> {
    timestamp
        .try_into()
        .map_err(|_| ErrorCode::InvalidTimestamp)
}

pub fn is_stale(current_time: u64, result_time: u64, max_age_seconds: u64) -> bool {
    current_time.saturating_sub(result_time) > max_age_seconds
}

pub fn validate_price(e: &Env, price: &PythPrice, config: &OracleConfig) -> Result<(), ErrorCode> {
    let current_time = e.ledger().timestamp();
    let publish_time = cast_external_timestamp(price.publish_time)?;

    // Check freshness
    if is_stale(current_time, publish_time, config.max_staleness_seconds) {
/// Issue #41: Use saturating_abs to avoid overflow on i64::MIN.
/// Issue #49: publish_time is now u64 — no signed/unsigned mixing.
pub fn validate_price(e: &Env, price: &PythPrice, config: &OracleConfig) -> Result<(), ErrorCode> {
    let current_time = e.ledger().timestamp(); // u64
    let age = current_time.saturating_sub(price.publish_time);

    let max_staleness = config.max_staleness_seconds;
    if age > max_staleness {
        return Err(ErrorCode::StalePrice);
    }

    // Check confidence: conf should be < max_confidence_bps% of price
    let price_abs = if price.price < 0 {
        (-price.price) as u64
    } else {
        price.price
    } as u64;
    let max_conf = (price_abs * config.max_confidence_bps) / 10000;
    let max_conf = (price_abs * config.max_confidence_bps as u64) / 10000;

    if price.conf > max_conf {
        return Err(ErrorCode::ConfidenceTooLow);
    }

    Ok(())
}

pub fn resolve_with_pyth(
    e: &Env,
    market_id: u64,
    oracle_id: u32,
    config: &OracleConfig,
) -> Result<u32, ErrorCode> {
    let price = fetch_pyth_price(e, config)?;
    let publish_time = cast_external_timestamp(price.publish_time)?;

    // Convert price to outcome (implementation depends on market logic)
    let outcome = determine_outcome(&price);

    // Store result
    e.storage()
        .persistent()
        .set(&OracleData::Result(market_id, 0), &outcome);
    e.storage()
        .persistent()
        .set(&OracleData::LastUpdate(market_id, 0), &publish_time);

    // Publish event
    validate_price(e, &price, config)?;

    let outcome = determine_outcome(&price);

    e.storage()
        .persistent()
        .set(&OracleData::Result(market_id, oracle_id), &outcome);
    e.storage().persistent().set(
        &OracleData::LastUpdate(market_id, oracle_id),
        &price.publish_time,
    );

    e.events().publish(
        (symbol_short!("oracle_ok"), market_id),
        (outcome, price.price, price.conf),
    );

    Ok(outcome)
}

fn determine_outcome(price: &PythPrice) -> u32 {
    // Placeholder logic - real implementation would use market-specific threshold
    if price.price > 0 {
        0
    } else {
        1
    }
    if price.price > 0 { 0 } else { 1 }
}

/// Issue #9: oracle_id parameter added; callers use 0 for the primary oracle.
pub fn get_oracle_result(e: &Env, market_id: u64, oracle_id: u32) -> Option<u32> {
    e.storage()
        .persistent()
        .get(&OracleData::Result(market_id, oracle_id))
}

pub fn set_oracle_result(e: &Env, market_id: u64, outcome: u32) -> Result<(), ErrorCode> {
    e.storage()
        .persistent()
        .set(&OracleData::Result(market_id, 0), &outcome);
    e.storage().persistent().set(
        &OracleData::LastUpdate(market_id, 0),
        &e.ledger().timestamp(),
    );

    let oracle_addr = e.current_contract_address();
    crate::modules::events::emit_oracle_result_set(e, market_id, oracle_addr, outcome);

    Ok(())
}

pub fn verify_oracle_health(_e: &Env, config: &OracleConfig) -> bool {
    !config.feed_id.is_empty()
}

#[cfg(test)]
mod tests {
    use super::abs_price_to_u64;

    #[test]
    fn abs_price_handles_i64_min_without_panic() {
        assert_eq!(abs_price_to_u64(i64::MIN), i64::MAX as u64);
    }

    #[test]
    fn abs_price_preserves_normal_values() {
        assert_eq!(abs_price_to_u64(-123), 123);
        assert_eq!(abs_price_to_u64(456), 456);
    }
}
