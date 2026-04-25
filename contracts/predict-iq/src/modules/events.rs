use soroban_sdk::{symbol_short, Address, Env};

/// Standardized Event Emission Module
///
/// Event Topic Layout:
/// - Topic 0: Event Name (short symbol, max 9 chars)
/// - Topic 1: market_id (u64) - primary identifier for indexers
/// - Topic 2: Triggering Address - who initiated the action
///
/// This standardization ensures external indexers can perfectly reconstruct
/// market states by following a consistent event schema.
///
/// EVENT SCHEMA VERSION: 1.0
/// Last Updated: 2026-04-25
///
/// Indexer Integration Guide:
/// 1. Subscribe to contract events using market_id as primary filter
/// 2. Process events in ledger order to maintain state consistency
/// 3. For event replay: query all events for a market_id and replay sequentially
/// 4. All timestamps are in Unix seconds (ledger.timestamp())
/// 5. All amounts are in stroops (1 XLM = 10^7 stroops)
///
/// Event Replay Support:
/// - All events are immutable once emitted
/// - Events include sufficient context for state reconstruction
/// - market_id is always present for efficient indexing
/// - Timestamps enable temporal queries

pub fn emit_market_created(
    e: &Env,
    market_id: u64,
    creator: Address,
    description: soroban_sdk::String,
    num_outcomes: u32,
    deadline: u64,
) {
    e.events().publish(
        (symbol_short!("mkt_creat"), market_id, creator),
        (description, num_outcomes, deadline),
    );
}

pub fn emit_bet_placed(e: &Env, market_id: u64, bettor: Address, outcome: u32, amount: i128) {
    e.events().publish(
        (symbol_short!("bet_place"), market_id, bettor),
        (outcome, amount),
    );
}

pub fn emit_dispute_filed(e: &Env, market_id: u64, disciplinarian: Address, new_deadline: u64) {
    e.events().publish(
        (symbol_short!("disp_file"), market_id, disciplinarian),
        new_deadline,
    );
}

pub fn emit_resolution_finalized(
    e: &Env,
    market_id: u64,
    resolver: Address,
    winning_outcome: u32,
    total_payout: i128,
) {
    e.events().publish(
        (symbol_short!("resolv_fx"), market_id, resolver),
        (winning_outcome, total_payout),
    );
}

pub fn emit_rewards_claimed(
    e: &Env,
    market_id: u64,
    claimer: Address,
    amount: i128,
    token_address: Address,
    is_refund: bool,
) {
    e.events().publish(
        (symbol_short!("reward_fx"), market_id, claimer),
        (amount, token_address, is_refund),
    );
}

pub fn emit_vote_cast(e: &Env, market_id: u64, voter: Address, outcome: u32, weight: i128) {
    e.events().publish(
        (symbol_short!("vote_cast"), market_id, voter),
        (outcome, weight),
    );
}

pub fn emit_circuit_breaker_triggered(
    e: &Env,
    contract_address: Address,
    state: soroban_sdk::String,
) {
    e.events()
        .publish((symbol_short!("cb_state"), 0u64, contract_address), state);
}

/// Emit OracleResultSet event.
///
/// Issue #405: Event now includes oracle_id and oracle_source (the actual oracle
/// contract address from OracleConfig) instead of the current contract address.
///
/// Indexer schema:
///   topics: [oracle_ok, market_id, oracle_source: Address]
///   data:   (oracle_id: u32, outcome: u32)
pub fn emit_oracle_result_set(
    e: &Env,
    market_id: u64,
    oracle_id: u32,
    oracle_source: Address,
    outcome: u32,
) {
    e.events().publish(
        (symbol_short!("oracle_ok"), market_id, oracle_source),
        (oracle_id, outcome),
    );
}

pub fn emit_oracle_resolved(e: &Env, market_id: u64, oracle_address: Address, outcome: u32) {
    e.events().publish(
        (symbol_short!("orcl_res"), market_id, oracle_address),
        outcome,
    );
}

pub fn emit_market_finalized(e: &Env, market_id: u64, resolver: Address, winning_outcome: u32) {
    e.events().publish(
        (symbol_short!("mkt_final"), market_id, resolver),
        winning_outcome,
    );
}

pub fn emit_dispute_resolved(e: &Env, market_id: u64, resolver: Address, winning_outcome: u32) {
    e.events().publish(
        (symbol_short!("disp_res"), market_id, resolver),
        winning_outcome,
    );
}

pub fn emit_market_cancelled(e: &Env, market_id: u64, admin: Address) {
    e.events()
        .publish((symbol_short!("mkt_cncl"), market_id, admin), ());
}

pub fn emit_market_cancelled_vote(e: &Env, market_id: u64, resolver: Address) {
    e.events()
        .publish((symbol_short!("mk_cn_vt"), market_id, resolver), ());
}

pub fn emit_referral_reward(e: &Env, market_id: u64, referrer: Address, amount: i128) {
    e.events()
        .publish((symbol_short!("ref_rwrd"), market_id, referrer), amount);
}

pub fn emit_referral_claimed(e: &Env, market_id: u64, claimer: Address, amount: i128) {
    e.events()
        .publish((symbol_short!("ref_claim"), market_id, claimer), amount);
}

pub fn emit_circuit_breaker_auto(e: &Env, contract_address: Address, error_count: u32) {
    e.events().publish(
        (symbol_short!("cb_auto"), 0u64, contract_address),
        error_count,
    );
}

pub fn emit_fee_collected(e: &Env, _market_id: u64, contract_address: Address, amount: i128) {
    e.events()
        .publish((symbol_short!("fee_colct"), 0u64, contract_address), amount);
}

/// Issue #63: Emit AdminFallbackResolution event
pub fn emit_admin_fallback_resolution(
    e: &Env,
    market_id: u64,
    admin: Address,
    winning_outcome: u32,
) {
    e.events().publish(
        (symbol_short!("adm_fbk"), market_id, admin),
        winning_outcome,
    );
}

pub fn emit_creator_reputation_set(e: &Env, creator: Address, old_score: u32, new_score: u32) {
    e.events().publish(
        (symbol_short!("rep_set"), creator),
        (old_score, new_score),
    );
}

pub fn emit_creation_deposit_set(e: &Env, old_amount: i128, new_amount: i128) {
    e.events().publish(
        (symbol_short!("dep_set"),),
        (old_amount, new_amount),
    );
}

pub fn emit_monitoring_state_reset(
    e: &Env,
    resetter: Address,
    previous_error_count: u32,
    previous_last_observation: u64,
) {
    e.events().publish(
        (symbol_short!("mon_reset"), resetter),
        (previous_error_count, previous_last_observation),
    );
}

pub fn emit_market_pruned(e: &Env, market_id: u64, pruned_at: u64) {
    e.events().publish(
        (symbol_short!("mkt_prune"), market_id),
        pruned_at,
    );
}

pub fn emit_upgrade_initiated(e: &Env, initiator: Address, wasm_hash: soroban_sdk::BytesN<32>) {
    e.events().publish(
        (symbol_short!("upg_init"), initiator),
        wasm_hash,
    );
}

pub fn emit_upgrade_voted(e: &Env, voter: Address, vote_for: bool) {
    e.events().publish(
        (symbol_short!("upg_vote"), voter),
        vote_for,
    );
}

pub fn emit_upgrade_executed(e: &Env, executor: Address, wasm_hash: soroban_sdk::BytesN<32>) {
    e.events().publish(
        (symbol_short!("upg_exec"), executor),
        wasm_hash,
    );
}

pub fn emit_upgrade_rejected(e: &Env, wasm_hash: soroban_sdk::BytesN<32>) {
    e.events().publish(
        (symbol_short!("upg_rej"),),
        wasm_hash,
    );
}

/// Issue #506: Emit MarketStateChanged event for indexing
/// Includes all fields needed for off-chain state reconstruction
pub fn emit_market_state_changed(
    e: &Env,
    market_id: u64,
    old_status: soroban_sdk::String,
    new_status: soroban_sdk::String,
    timestamp: u64,
) {
    e.events().publish(
        (symbol_short!("mkt_state"), market_id),
        (old_status, new_status, timestamp),
    );
}
