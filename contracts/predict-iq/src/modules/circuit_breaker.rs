use soroban_sdk::{Env, Symbol};
use crate::types::{ConfigKey, CircuitBreakerState};
use crate::modules::admin;
use crate::errors::ErrorCode;

pub fn set_state(e: &Env, state: CircuitBreakerState) -> Result<(), ErrorCode> {
    admin::require_admin(e)?;
    e.storage().persistent().set(&ConfigKey::CircuitBreakerState, &state);

    // Event format: (Topic, MarketID, SubjectAddr, Data) - no market_id for global state
    e.events().publish(
        (Symbol::new(e, "circuit_breaker_updated"),),
        state,
    );
    
    Ok(())
}

pub fn get_state(e: &Env) -> CircuitBreakerState {
    e.storage().persistent().get(&ConfigKey::CircuitBreakerState).unwrap_or(CircuitBreakerState::Closed)
}

pub fn require_closed(e: &Env) -> Result<(), ErrorCode> {
    if get_state(e) == CircuitBreakerState::Open {
        return Err(ErrorCode::CircuitBreakerOpen);
    }
    Ok(())
}
