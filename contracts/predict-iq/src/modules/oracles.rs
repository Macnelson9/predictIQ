use crate::errors::ErrorCode;
use crate::types::OracleConfig;
use soroban_sdk::{contracttype, symbol_short, Env, Map};

#[contracttype]
pub enum OracleData {
    Result(u64, u32),     // market_id -> outcome
    LastUpdate(u64, u64), // market_id -> timestamp
    OracleResponses(u64), // market_id -> Map<oracle_index, outcome>
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PythPrice {
    pub price: i64,
    pub conf: u64,
    pub expo: i32,
    pub publish_time: i64,
}

pub fn fetch_pyth_price(_e: &Env, _config: &OracleConfig) -> Result<PythPrice, ErrorCode> {
    // In production, this would call the Pyth contract
    // For now, return a mock implementation that can be overridden in tests
    Err(ErrorCode::OracleFailure)
}

pub fn validate_price(e: &Env, price: &PythPrice, config: &OracleConfig) -> Result<(), ErrorCode> {
    let current_time = e.ledger().timestamp() as i64;
    let age = current_time - price.publish_time;

    // Check freshness - Issue #508: Enforce staleness validation
    if age > config.max_staleness_seconds as i64 {
        return Err(ErrorCode::StalePrice);
    }

    // Check confidence: conf should be < max_confidence_bps% of price
    let price_abs = if price.price < 0 {
        -price.price
    } else {
        price.price
    } as u64;
    let max_conf = (price_abs * config.max_confidence_bps) / 10000;

    if price.conf > max_conf {
        return Err(ErrorCode::ConfidenceTooLow);
    }

    Ok(())
}

/// Issue #508: Validate oracle staleness before resolution
pub fn validate_oracle_staleness(
    e: &Env,
    market_id: u64,
    config: &OracleConfig,
) -> Result<(), ErrorCode> {
    let last_update = e
        .storage()
        .persistent()
        .get::<_, u64>(&OracleData::LastUpdate(market_id, 0));

    if let Some(update_time) = last_update {
        let current_time = e.ledger().timestamp();
        let age = current_time - update_time;

        // Check if oracle data is stale
        if age > config.max_staleness_seconds {
            return Err(ErrorCode::StalePrice);
        }
        Ok(())
    } else {
        // No oracle data available
        Err(ErrorCode::OracleFailure)
    }
}

pub fn resolve_with_pyth(e: &Env, market_id: u64, config: &OracleConfig) -> Result<u32, ErrorCode> {
    let price = fetch_pyth_price(e, config)?;

    // Convert price to outcome (implementation depends on market logic)
    let outcome = determine_outcome(&price);

    // Store result
    e.storage()
        .persistent()
        .set(&OracleData::Result(market_id, 0), &outcome);
    e.storage().persistent().set(
        &OracleData::LastUpdate(market_id, 0),
        &(price.publish_time as u64),
    );

    // Publish event with real oracle source from config
    e.events().publish(
        (symbol_short!("oracle_ok"), market_id, config.oracle_address.clone()),
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
}

pub fn get_oracle_result(e: &Env, market_id: u64, _config: &OracleConfig) -> Option<u32> {
    // In a real implementation, this would call the external oracle contract (Reflector/Pyth)
    // using config.oracle_address and config.feed_id.
    // For this replication, we use a storage-backed mock-ready structure.
    e.storage()
        .persistent()
        .get(&OracleData::Result(market_id, 0)) // Note: 0 is dummy key part
}

pub fn set_oracle_result(e: &Env, market_id: u64, outcome: u32) -> Result<(), ErrorCode> {
    // Mock oracle result for testing/demonstration
    e.storage()
        .persistent()
        .set(&OracleData::Result(market_id, 0), &outcome);
    e.storage().persistent().set(
        &OracleData::LastUpdate(market_id, 0),
        &e.ledger().timestamp(),
    );

    // Emit event with real oracle source from market config
    let oracle_addr = crate::modules::markets::get_market(e, market_id)
        .map(|m| m.oracle_config.oracle_address)
        .unwrap_or_else(|| e.current_contract_address());
    crate::modules::events::emit_oracle_result_set(e, market_id, 0u32, oracle_addr, outcome);

    Ok(())
}

pub fn verify_oracle_health(_e: &Env, config: &OracleConfig) -> bool {
    !config.feed_id.is_empty()
}

/// Issue #509: Record an oracle response for consensus validation
pub fn record_oracle_response(
    e: &Env,
    market_id: u64,
    oracle_index: u32,
    outcome: u32,
) -> Result<(), ErrorCode> {
    let key = OracleData::OracleResponses(market_id);
    let mut responses: Map<u32, u32> = e
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Map::new(e));

    responses.set(oracle_index, outcome);
    e.storage().persistent().set(&key, &responses);

    Ok(())
}

/// Issue #509: Validate oracle consensus - requires min_responses confirmations
pub fn validate_consensus(
    e: &Env,
    market_id: u64,
    config: &OracleConfig,
) -> Result<u32, ErrorCode> {
    let min_responses = config.min_responses.unwrap_or(1);

    let key = OracleData::OracleResponses(market_id);
    let responses: Map<u32, u32> = e
        .storage()
        .persistent()
        .get(&key)
        .ok_or(ErrorCode::OracleFailure)?;

    // Check if we have enough responses
    if responses.len() < min_responses {
        return Err(ErrorCode::OracleFailure);
    }

    // Count votes for each outcome
    let mut outcome_votes: Map<u32, u32> = Map::new(e);
    let mut i = 0u32;
    while i < responses.len() {
        if let Some(outcome) = responses.get(i) {
            let votes = outcome_votes.get(outcome).unwrap_or(0);
            outcome_votes.set(outcome, votes + 1);
        }
        i += 1;
    }

    // Find outcome with most votes (quorum)
    let mut consensus_outcome: Option<u32> = None;
    let mut max_votes = 0u32;
    let mut i = 0u32;
    while i < outcome_votes.len() {
        if let Some(outcome) = outcome_votes.get(i) {
            if let Some(votes) = outcome_votes.get(outcome) {
                if votes > max_votes {
                    max_votes = votes;
                    consensus_outcome = Some(outcome);
                }
            }
        }
        i += 1;
    }

    consensus_outcome.ok_or(ErrorCode::OracleFailure)
}
