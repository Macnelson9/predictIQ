use soroban_sdk::{Env, Address, Symbol};
use crate::types::MarketStatus;
use crate::modules::markets;
use crate::errors::ErrorCode;

pub fn file_dispute(e: &Env, disciplinarian: Address, market_id: u64) -> Result<(), ErrorCode> {
    disciplinarian.require_auth();

    let mut market = markets::get_market(e, market_id).ok_or(ErrorCode::MarketNotFound)?;
    
    if market.status != MarketStatus::PendingResolution {
        return Err(ErrorCode::MarketNotPendingResolution);
    }

    market.status = MarketStatus::Disputed;
    // Extend resolution deadline for voting period
    market.resolution_deadline += 86400 * 3; // 3 days extension

    markets::update_market(e, market);

    // Event format: (Topic, MarketID, SubjectAddr, Data)
    e.events().publish(
        (Symbol::new(e, "market_disputed"), market_id, disciplinarian),
        (),
    );
    
    Ok(())
}

pub fn resolve_market(e: &Env, market_id: u64, winning_outcome: u32) -> Result<(), ErrorCode> {
    // This would ideally be called by an admin or triggered by oracle/consensus
    let mut market = markets::get_market(e, market_id).ok_or(ErrorCode::MarketNotFound)?;
    
    market.status = MarketStatus::Resolved;
    market.winning_outcome = Some(winning_outcome);

    markets::update_market(e, market);

    // Event format: (Topic, MarketID, SubjectAddr, Data)
    e.events().publish(
        (Symbol::new(e, "market_resolved"), market_id),
        winning_outcome,
    );
    
    Ok(())
}
