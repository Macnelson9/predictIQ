use soroban_sdk::{Env, Address, Symbol, contracttype, token};
use crate::types::{Bet, MarketStatus};
use crate::modules::markets;
use crate::errors::ErrorCode;

#[contracttype]
pub enum DataKey {
    Bet(u64, Address), // market_id, bettor
}

pub fn place_bet(
    e: &Env,
    bettor: Address,
    market_id: u64,
    outcome: u32,
    amount: i128,
    token_address: Address,
) -> Result<(), ErrorCode> {
    bettor.require_auth();

    let mut market = markets::get_market(e, market_id).ok_or(ErrorCode::MarketNotFound)?;
    
    if market.status != MarketStatus::Active {
        return Err(ErrorCode::MarketNotActive);
    }

    if e.ledger().timestamp() >= market.deadline {
        return Err(ErrorCode::DeadlinePassed);
    }

    if outcome >= market.options.len() {
        return Err(ErrorCode::InvalidOutcome);
    }

    // Transfer tokens from bettor to contract
    let client = token::Client::new(e, &token_address);
    client.transfer(&bettor, &e.current_contract_address(), &amount);

    let bet_key = DataKey::Bet(market_id, bettor.clone());
    let mut existing_bet: Bet = e.storage().persistent().get(&bet_key).unwrap_or(Bet {
        market_id,
        bettor: bettor.clone(),
        outcome,
        amount: 0,
    });

    if existing_bet.amount > 0 && existing_bet.outcome != outcome {
        return Err(ErrorCode::CannotChangeOutcome);
    }

    existing_bet.amount += amount;
    market.total_staked += amount;

    e.storage().persistent().set(&bet_key, &existing_bet);
    markets::update_market(e, market);

    // Event format: (Topic, MarketID, SubjectAddr, Data)
    e.events().publish(
        (Symbol::new(e, "bet_placed"), market_id, bettor),
        amount,
    );
    
    Ok(())
}

pub fn get_bet(e: &Env, market_id: u64, bettor: Address) -> Option<Bet> {
    e.storage().persistent().get(&DataKey::Bet(market_id, bettor))
}
