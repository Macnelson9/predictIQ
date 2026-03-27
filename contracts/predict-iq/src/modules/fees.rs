use crate::errors::ErrorCode;
use crate::modules::admin;
use crate::types::{ConfigKey, MarketTier, GOV_TTL_HIGH_THRESHOLD, GOV_TTL_LOW_THRESHOLD};
use soroban_sdk::{contracttype, Address, Env, Symbol};

const BPS_DENOMINATOR: i128 = 10_000;
const TIER_DENOMINATOR_BPS: i128 = 10_000;

#[contracttype]
pub enum DataKey {
    TotalFeesCollected,
    FeeRevenue(Address),
    /// Issue #1: Key is now (referrer, token) to prevent cross-asset mixing.
    ReferrerBalance(Address, Address),
}

fn bump_config_ttl(e: &Env, key: &ConfigKey) {
    e.storage()
        .persistent()
        .extend_ttl(key, GOV_TTL_LOW_THRESHOLD, GOV_TTL_HIGH_THRESHOLD);
}

pub fn get_base_fee(e: &Env) -> i128 {
    e.storage()
        .persistent()
        .get(&ConfigKey::BaseFee)
        .unwrap_or(0)
}

pub fn set_base_fee(e: &Env, amount: i128) -> Result<(), ErrorCode> {
    admin::require_fee_admin(e)?;
    e.storage().persistent().set(&ConfigKey::BaseFee, &amount);
    bump_config_ttl(e, &ConfigKey::BaseFee);
    Ok(())
}

pub fn calculate_fee(e: &Env, amount: i128) -> i128 {
    let base_fee = get_base_fee(e);
    amount.saturating_mul(base_fee) / BPS_DENOMINATOR
}

fn tier_multiplier_bps(tier: &MarketTier) -> i128 {
    match tier {
        MarketTier::Basic => TIER_DENOMINATOR_BPS,
        MarketTier::Pro => 7_500,           // 25% discount
        MarketTier::Institutional => 5_000, // 50% discount
    }
}

fn calculate_tiered_fee_with_base(amount: i128, base_fee_bps: i128, tier: &MarketTier) -> i128 {
    // Single-pass high-precision arithmetic: amount * base_fee_bps * tier_multiplier / (10_000 * 10_000)
    // This avoids early truncation from computing discounted base_fee first.
    let numerator = amount
        .saturating_mul(base_fee_bps)
        .saturating_mul(tier_multiplier_bps(tier));
    numerator / (BPS_DENOMINATOR * TIER_DENOMINATOR_BPS)
}

/// Issue #39: multiply before divide and keep tier multipliers in bps.
pub fn calculate_tiered_fee(e: &Env, amount: i128, tier: &MarketTier) -> i128 {
    let base_fee = get_base_fee(e);
    calculate_tiered_fee_with_base(amount, base_fee, tier)
}

pub fn collect_fee(e: &Env, token: Address, amount: i128) {
    let key = DataKey::FeeRevenue(token.clone());
    let mut total: i128 = e.storage().persistent().get(&key).unwrap_or(0);
    total += amount;
    e.storage().persistent().set(&key, &total);

    let mut overall: i128 = e
        .storage()
        .persistent()
        .get(&DataKey::TotalFeesCollected)
        .unwrap_or(0);
    overall += amount;
    e.storage()
        .persistent()
        .set(&DataKey::TotalFeesCollected, &overall);

    // Emit standardized fee collection event using centralized emitter
    let contract_addr = e.current_contract_address();
    crate::modules::events::emit_fee_collected(e, 0, contract_addr, amount);
}

pub fn get_revenue(e: &Env, token: Address) -> i128 {
    e.storage()
        .persistent()
        .get(&DataKey::FeeRevenue(token))
        .unwrap_or(0)
}

/// Issue #26: Allow FeeAdmin/Admin to withdraw accumulated protocol fees.
pub fn withdraw_protocol_fees(
    e: &Env,
    token: &Address,
    recipient: &Address,
) -> Result<i128, ErrorCode> {
    // Allow either admin or fee_admin to withdraw
    let is_admin = admin::get_admin(e)
        .map(|a| {
            a.try_require_auth().is_ok()
        })
        .unwrap_or(false);
    let is_fee_admin = admin::get_fee_admin(e)
        .map(|a| {
            a.try_require_auth().is_ok()
        })
        .unwrap_or(false);

    if !is_admin && !is_fee_admin {
        return Err(ErrorCode::NotAuthorized);
    }

    let key = DataKey::FeeRevenue(token.clone());
    let balance: i128 = e.storage().persistent().get(&key).unwrap_or(0);

    if balance == 0 {
        return Err(ErrorCode::InsufficientBalance);
    }

    e.storage().persistent().set(&key, &0i128);

    let client = soroban_sdk::token::Client::new(e, token);
    client.transfer(&e.current_contract_address(), recipient, &balance);

    e.events()
        .publish((Symbol::new(e, "fees_withdrawn"), recipient), balance);

    Ok(balance)
}

/// Issue #1: Referral reward keyed by (referrer, token) to prevent cross-asset mixing.
pub fn add_referral_reward(e: &Env, referrer: &Address, token: &Address, fee_amount: i128) {
    let reward = (fee_amount * 10) / 100;
    let key = DataKey::ReferrerBalance(referrer.clone(), token.clone());
    let mut balance: i128 = e.storage().persistent().get(&key).unwrap_or(0);
    balance += reward;
    e.storage().persistent().set(&key, &balance);

    crate::modules::events::emit_referral_reward(e, 0, referrer.clone(), reward);
}

/// Issue #1: Claim referral rewards for a specific token only.
pub fn claim_referral_rewards(
    e: &Env,
    address: &Address,
    token: &Address,
) -> Result<i128, ErrorCode> {
    address.require_auth();

    let key = DataKey::ReferrerBalance(address.clone(), token.clone());
    let balance: i128 = e.storage().persistent().get(&key).unwrap_or(0);

    if balance == 0 {
        return Err(ErrorCode::InsufficientBalance);
    }

    e.storage().persistent().set(&key, &0i128);

    let client = soroban_sdk::token::Client::new(e, token);
    e.current_contract_address().require_auth();
    client.transfer(&e.current_contract_address(), address, &balance);

    crate::modules::events::emit_referral_claimed(e, 0, address.clone(), balance);

    Ok(balance)
}

#[cfg(test)]
mod tests {
    use super::{calculate_tiered_fee_with_base, MarketTier};

    #[test]
    fn tiered_fee_keeps_fractional_discount_precision() {
        // 1 bps base fee with Pro tier (25% discount):
        // old math: ((1 * 75) / 100) = 0 bps => zero fee for all amounts.
        // new math preserves the discounted 0.75 bps effect until final division.
        let basic_fee = calculate_tiered_fee_with_base(4_000_000, 1, &MarketTier::Basic);
        let pro_fee = calculate_tiered_fee_with_base(4_000_000, 1, &MarketTier::Pro);
        assert_eq!(basic_fee, 400);
        assert_eq!(pro_fee, 300);
    }

    #[test]
    fn tiered_fee_uses_expected_discount_ratio() {
        let basic_fee = calculate_tiered_fee_with_base(10_000, 100, &MarketTier::Basic);
        let pro_fee = calculate_tiered_fee_with_base(10_000, 100, &MarketTier::Pro);
        let inst_fee = calculate_tiered_fee_with_base(10_000, 100, &MarketTier::Institutional);

        assert_eq!(basic_fee, 100);
        assert_eq!(pro_fee, 75);
        assert_eq!(inst_fee, 50);
    }

    #[test]
    fn four_unit_bet_applies_pro_discount() {
        let basic_fee = calculate_tiered_fee_with_base(4, 10_000, &MarketTier::Basic);
        let pro_fee = calculate_tiered_fee_with_base(4, 10_000, &MarketTier::Pro);

        assert_eq!(basic_fee, 4);
        assert_eq!(pro_fee, 3);
    }
}
