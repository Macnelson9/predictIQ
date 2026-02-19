# PredictIQ API Specification

## Error Codes

All public contract methods return `Result<T, ErrorCode>` where `ErrorCode` is a stable enumeration of error conditions. Frontend applications should handle these error codes appropriately.

### Error Code Reference

| Code | Name | Description | Recommended Action |
|------|------|-------------|-------------------|
| 100 | AlreadyInitialized | Contract has already been initialized | Check initialization state before calling |
| 101 | NotAuthorized | Caller lacks required authorization | Verify authentication and permissions |
| 102 | MarketNotFound | Requested market does not exist | Verify market ID exists |
| 103 | MarketClosed | Market is closed for betting | Display market status to user |
| 104 | MarketStillActive | Market is still active (cannot resolve yet) | Wait for market deadline |
| 105 | InvalidOutcome | Outcome index is out of bounds | Validate outcome against market options |
| 106 | InvalidBetAmount | Bet amount is invalid (zero or negative) | Validate bet amount > 0 |
| 107 | InsufficientBalance | User has insufficient balance | Check user balance before transaction |
| 108 | OracleFailure | Oracle failed to provide result | Retry or use manual resolution |
| 109 | CircuitBreakerOpen | Circuit breaker is open (system paused) | Display maintenance message |
| 110 | DisputeWindowClosed | Dispute period has ended | Inform user dispute is no longer possible |
| 111 | VotingNotStarted | Voting period has not begun | Wait for voting to start |
| 112 | VotingEnded | Voting period has ended | Inform user voting is closed |
| 113 | AlreadyVoted | User has already cast a vote | Display existing vote |
| 114 | FeeTooHigh | Fee exceeds acceptable threshold | Review fee configuration |
| 115 | MarketNotActive | Market is not in active state | Check market status |
| 116 | DeadlinePassed | Market deadline has passed | Inform user betting is closed |
| 117 | CannotChangeOutcome | Cannot change bet outcome after initial bet | Inform user outcome is locked |
| 118 | MarketNotDisputed | Market is not in disputed state | Check market status |
| 119 | MarketNotPendingResolution | Market is not pending resolution | Check market status |
| 120 | AdminNotSet | Admin address has not been configured | Initialize contract first |

## Event Schema

All events follow a standardized format to minimize gas costs while maintaining essential information for frontend integration.

### Event Format

```rust
(Topic, MarketID, SubjectAddr, Data)
```

- **Topic**: Symbol identifying the event type
- **MarketID**: u64 market identifier (0 for global events)
- **SubjectAddr**: Address of the primary actor (optional for some events)
- **Data**: Minimal event-specific data

### Event Types

#### Market Events

**market_created**
- Topics: `("market_created", market_id, creator_address)`
- Data: `()`
- Description: New market has been created

**market_disputed**
- Topics: `("market_disputed", market_id, disputer_address)`
- Data: `()`
- Description: Market outcome has been disputed

**market_resolved**
- Topics: `("market_resolved", market_id)`
- Data: `winning_outcome: u32`
- Description: Market has been resolved with winning outcome

#### Betting Events

**bet_placed**
- Topics: `("bet_placed", market_id, bettor_address)`
- Data: `amount: i128`
- Description: Bet has been placed on a market

#### Voting Events

**vote_cast**
- Topics: `("vote_cast", market_id, voter_address)`
- Data: `outcome: u32`
- Description: Vote has been cast in a disputed market

#### Oracle Events

**oracle_update**
- Topics: `("oracle_update", market_id)`
- Data: `outcome: u32`
- Description: Oracle has provided a result

#### System Events

**circuit_breaker_updated**
- Topics: `("circuit_breaker_updated")`
- Data: `CircuitBreakerState`
- Description: Circuit breaker state has changed

**automatic_circuit_breaker_trigger**
- Topics: `("automatic_circuit_breaker_trigger")`
- Data: `error_count: u32`
- Description: Circuit breaker automatically triggered due to errors

**fee_collected**
- Topics: `("fee_collected")`
- Data: `amount: i128`
- Description: Fee has been collected

## Public Methods

### Initialization

#### `initialize(admin: Address, base_fee: i128) -> Result<(), ErrorCode>`
Initialize the contract with admin and base fee configuration.

**Errors:**
- `AlreadyInitialized` (100): Contract already initialized

### Market Management

#### `create_market(...) -> Result<u64, ErrorCode>`
Create a new prediction market.

**Returns:** Market ID on success

**Errors:**
- `NotAuthorized` (101): Caller not authorized

#### `get_market(id: u64) -> Option<Market>`
Retrieve market details by ID.

**Returns:** Market data or None if not found

### Betting

#### `place_bet(bettor: Address, market_id: u64, outcome: u32, amount: i128, token_address: Address) -> Result<(), ErrorCode>`
Place a bet on a market outcome.

**Errors:**
- `MarketNotFound` (102): Market does not exist
- `MarketNotActive` (115): Market is not accepting bets
- `DeadlinePassed` (116): Betting deadline has passed
- `InvalidOutcome` (105): Invalid outcome index
- `CannotChangeOutcome` (117): Cannot change existing bet outcome

### Voting

#### `cast_vote(voter: Address, market_id: u64, outcome: u32, weight: i128) -> Result<(), ErrorCode>`
Cast a vote in a disputed market.

**Errors:**
- `CircuitBreakerOpen` (109): System is paused
- `MarketNotFound` (102): Market does not exist
- `MarketNotDisputed` (118): Market is not in dispute
- `InvalidOutcome` (105): Invalid outcome index
- `AlreadyVoted` (113): User has already voted

### Disputes

#### `file_dispute(disciplinarian: Address, market_id: u64) -> Result<(), ErrorCode>`
File a dispute for a market resolution.

**Errors:**
- `CircuitBreakerOpen` (109): System is paused
- `MarketNotFound` (102): Market does not exist
- `MarketNotPendingResolution` (119): Market not in correct state

### Administration

#### `set_circuit_breaker(state: CircuitBreakerState) -> Result<(), ErrorCode>`
Update circuit breaker state.

**Errors:**
- `NotAuthorized` (101): Caller is not admin
- `AdminNotSet` (120): Admin not configured

#### `set_base_fee(amount: i128) -> Result<(), ErrorCode>`
Update base fee configuration.

**Errors:**
- `NotAuthorized` (101): Caller is not admin
- `AdminNotSet` (120): Admin not configured

#### `set_oracle_result(market_id: u64, outcome: u32) -> Result<(), ErrorCode>`
Set oracle result for a market (admin only).

**Errors:**
- `NotAuthorized` (101): Caller is not admin
- `AdminNotSet` (120): Admin not configured

#### `reset_monitoring() -> Result<(), ErrorCode>`
Reset monitoring error counters (admin only).

**Errors:**
- `NotAuthorized` (101): Caller is not admin
- `AdminNotSet` (120): Admin not configured

### Query Methods

#### `get_admin() -> Option<Address>`
Get the current admin address.

#### `get_revenue(token: Address) -> i128`
Get total revenue collected for a specific token.

## Frontend Integration Guide

### Error Handling

```typescript
// Example error handling in TypeScript
function handleContractError(errorCode: number): string {
  const errorMessages: Record<number, string> = {
    100: "Contract already initialized",
    101: "Not authorized to perform this action",
    102: "Market not found",
    // ... add all error codes
  };
  
  return errorMessages[errorCode] || "Unknown error occurred";
}
```

### Event Parsing

```typescript
// Example event parsing
interface BetPlacedEvent {
  topic: string;
  marketId: bigint;
  bettor: string;
  amount: bigint;
}

function parseBetPlacedEvent(event: ContractEvent): BetPlacedEvent {
  return {
    topic: event.topics[0],
    marketId: event.topics[1],
    bettor: event.topics[2],
    amount: event.data
  };
}
```

## Version History

- **v1.0.0** (2024): Initial API specification with standardized error codes and event schema
