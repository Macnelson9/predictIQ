# PredictIQ Contract API Specification

> Reflects the on-chain implementation as of the current `contracts/predict-iq` source.
> **Spec version:** 1.1.0 — updated 2026-04-27 (issues #485: error code values corrected to match `#[repr(u32)]` enum; events table expanded with missing events and corrected topic layouts)

---

## Table of Contents

1. [Initialization](#initialization)
2. [Market Lifecycle](#market-lifecycle)
3. [Betting](#betting)
4. [Oracle & Resolution](#oracle--resolution)
5. [Disputes & Voting](#disputes--voting)
6. [Governance & Upgrades](#governance--upgrades)
7. [Fees & Referrals](#fees--referrals)
8. [Circuit Breaker](#circuit-breaker)
9. [Queries (Paginated)](#queries-paginated)
10. [Error Codes](#error-codes)
11. [Events](#events)

---

## Initialization

### `initialize(admin: Address, base_fee: i128) → Result<(), ErrorCode>`

Bootstraps the contract. Can only be called once.

| Param | Type | Description |
|-------|------|-------------|
| `admin` | `Address` | Master admin account (must authorize) |
| `base_fee` | `i128` | Protocol fee in stroops |

**Errors:** `AlreadyInitialized`

---

## Market Lifecycle

### `create_market(creator, description, options, deadline, resolution_deadline, oracle_config, tier, native_token, parent_id, parent_outcome_idx) → Result<u64, ErrorCode>`

| Param | Type | Description |
|-------|------|-------------|
| `creator` | `Address` | Market creator (must authorize) |
| `description` | `String` | Human-readable market question |
| `options` | `Vec<String>` | Outcome labels (max `MAX_OUTCOMES_PER_MARKET = 32`) |
| `deadline` | `u64` | Unix timestamp — betting closes |
| `resolution_deadline` | `u64` | Unix timestamp — resolution must occur by |
| `oracle_config` | `OracleConfig` | Multi-oracle configuration (see below) |
| `tier` | `MarketTier` | `Basic` \| `Pro` \| `Institutional` |
| `native_token` | `Address` | SAC token used for bets |
| `parent_id` | `u64` | `0` for independent markets; parent market ID for conditional |
| `parent_outcome_idx` | `u32` | Required parent outcome (ignored when `parent_id = 0`) |

**Returns:** new `market_id`

**OracleConfig fields:**

| Field | Type | Description |
|-------|------|-------------|
| `oracle_address` | `Address` | Deployed Pyth contract address |
| `feed_id` | `String` | 64-char hex-encoded 32-byte Pyth price feed ID |
| `min_responses` | `Option<u32>` | Minimum oracle responses required; `None` defaults to 1 |
| `max_staleness_seconds` | `u64` | Max age of price data in seconds |
| `max_confidence_bps` | `u64` | Max confidence interval in basis points |

**Errors:** `InvalidDeadline`, `TooManyOutcomes`, `InsufficientDeposit`, `MarketIdOverflow`, `MarketIdCollision`, `ParentMarketNotResolved`, `ParentMarketInvalidOutcome`

---

### `get_market(id: u64) → Option<Market>`

Returns the full `Market` struct or `None` if not found.

---

### `cancel_market_admin(market_id: u64) → Result<(), ErrorCode>`

Admin-only hard cancellation. Emits `mkt_cncl`.

**Errors:** `NotAuthorized`, `MarketNotFound`

---

### `prune_market(market_id: u64) → Result<(), ErrorCode>`

Permissionless cleanup after the 30-day grace period post-resolution.

**Errors:** `MarketNotFound`, `MarketStillActive`, `MarketNotResolved`

---

### `set_creator_reputation(creator: Address, reputation: CreatorReputation) → Result<(), ErrorCode>`

Admin-only. Sets `None | Basic | Pro | Institutional`.

---

### `set_creation_deposit(amount: i128) → Result<(), ErrorCode>` / `get_creation_deposit() → i128`

Admin-only deposit required to create a market.

---

### `claim_creation_deposit(market_id: u64, caller: Address) → Result<(), ErrorCode>`

Creator reclaims deposit after the dispute window closes without a challenge.

**Errors:** `MarketNotFound`, `NotAuthorized`, `DisputeWindowStillOpen`, `MarketNotDisputed`

---

## Betting

### `place_bet(bettor, market_id, outcome, amount, token_address, referrer) → Result<(), ErrorCode>`

| Param | Type | Description |
|-------|------|-------------|
| `bettor` | `Address` | Must authorize |
| `market_id` | `u64` | Target market |
| `outcome` | `u32` | Zero-based outcome index |
| `amount` | `i128` | Gross bet amount in token units |
| `token_address` | `Address` | Must match market's `token_address` |
| `referrer` | `Option<Address>` | Optional referral address |

**Errors:** `MarketNotFound`, `MarketClosed`, `MarketNotActive`, `InvalidBetAmount`, `InvalidOutcome`, `ContractPaused`, `InvalidReferrer`, `AssetClawedBack`, `TransferFailed`

---

### `claim_winnings(bettor: Address, market_id: u64) → Result<i128, ErrorCode>`

Pull-model payout. Returns amount transferred.

**Errors:** `MarketNotFound`, `MarketNotResolved`, `BetNotFound`, `NoWinnings`, `AlreadyClaimed`

---

### `withdraw_refund(bettor: Address, market_id: u64) → Result<i128, ErrorCode>`

Refund on cancelled markets.

**Errors:** `MarketNotFound`, `BetNotFound`, `AlreadyClaimed`

---

### `get_outcome_stake(market_id: u64, outcome: u32) → i128`

Total staked on a specific outcome.

---

### `count_bets_for_outcome(market_id: u64, outcome: u32) → u32`

Unique bettor count per outcome (analytics).

---

### `get_minimum_bet_amount() → i128` / `set_minimum_bet_amount(amount: i128) → Result<(), ErrorCode>`

---

## Oracle & Resolution

### `set_oracle_result(market_id: u64, oracle_id: u32, outcome: u32) → Result<(), ErrorCode>`

Admin-only. `oracle_id = 0` is the primary oracle. Supports multiple oracle sources per market.

**Errors:** `NotAuthorized`, `MarketNotFound`

---

### `get_oracle_result(market_id: u64, oracle_id: u32) → Option<u32>`

### `get_oracle_last_update(market_id: u64, oracle_id: u32) → Option<u64>`

---

### `attempt_oracle_resolution(market_id: u64) → Result<(), ErrorCode>`

Permissionless. Reads the oracle result and transitions the market to `PendingResolution` if conditions are met.

**Errors:** `MarketNotFound`, `MarketNotActive`, `OracleFailure`, `StalePrice`, `ConfidenceTooLow`, `ResolutionNotReady`

---

### `finalize_resolution(market_id: u64) → Result<(), ErrorCode>`

Permissionless. Moves `PendingResolution → Resolved` after the grace period.

**Errors:** `MarketNotFound`, `MarketNotPendingResolution`, `GracePeriodActive`, `ResolutionDeadlinePassed`

---

### `resolve_market(market_id: u64, winning_outcome: u32) → Result<(), ErrorCode>`

Admin-only resolution for disputed markets.

**Errors:** `NotAuthorized`, `MarketNotFound`, `MarketNotDisputed`

---

### `admin_fallback_resolution(market_id: u64, winning_outcome: u32) → Result<(), ErrorCode>`

Admin fallback when community voting deadlocks (no 60% majority after 72-hour window).

**Errors:** `NotAuthorized`, `MarketNotFound`, `MarketNotDisputed`, `VotingPeriodNotElapsed`, `NoMajorityReached`

---

### `set_dispute_window(seconds: u64) → Result<(), ErrorCode>` / `get_dispute_window() → u64`

Admin-only. Minimum 24 hours. Default 72 hours.

---

## Disputes & Voting

### `file_dispute(disciplinarian: Address, market_id: u64) → Result<(), ErrorCode>`

Opens a dispute window. Requires contract to be unpaused.

**Errors:** `MarketNotFound`, `MarketNotPendingResolution`, `DisputeWindowClosed`, `ContractPaused`

---

### `cast_vote(voter, market_id, outcome, weight) → Result<(), ErrorCode>`

Governance token holders vote on disputed outcome. Requires contract to be unpaused.

**Errors:** `MarketNotFound`, `MarketNotDisputed`, `AlreadyVoted`, `InsufficientVotingWeight`, `GovernanceTokenNotSet`, `ContractPaused`

---

### `unlock_tokens(voter: Address, market_id: u64) → Result<(), ErrorCode>`

Releases locked governance tokens after voting concludes.

---

### `get_resolution_metrics(market_id: u64, outcome: u32) → ResolutionMetrics`

### `set_max_push_payout_winners(threshold: u32)` / `get_max_push_payout_winners() → u32`

---

## Governance & Upgrades

### `add_guardian(guardian: Guardian) → Result<(), ErrorCode>`

### `remove_guardian(address: Address) → Result<(), ErrorCode>`

### `vote_on_guardian_removal(voter: Address, approve: bool) → Result<(), ErrorCode>`

### `get_guardians() → Vec<Guardian>`

### `emergency_pause(voter: Address) → Result<(), ErrorCode>`

Triggered by 2/3 Guardian majority.

---

### `initiate_upgrade(wasm_hash: BytesN<32>) → Result<(), ErrorCode>`

### `vote_for_upgrade(voter: Address, vote_for: bool) → Result<bool, ErrorCode>`

### `execute_upgrade() → Result<(), ErrorCode>`

### `get_pending_upgrade() → Option<PendingUpgrade>`

### `get_upgrade_votes() → Result<UpgradeStats, ErrorCode>`

Returns `{ votes_for: u32, votes_against: u32 }`.

### `is_timelock_satisfied() → Result<bool, ErrorCode>`

### `set_timelock_duration(seconds: u64) → Result<(), ErrorCode>` / `get_timelock_duration() → u64`

Range: 6 hours – 7 days. Default: 48 hours.

**Errors:** `TimelockActive`, `UpgradeNotInitiated`, `AlreadyVotedOnUpgrade`, `UpgradeAlreadyPending`, `UpgradeHashInCooldown`

---

### `set_guardian(guardian: Address) → Result<(), ErrorCode>` / `get_guardian() → Option<Address>`

Legacy single-guardian slot.

---

### `set_governance_token(token: Address) → Result<(), ErrorCode>`

---

## Fees & Referrals

### `set_base_fee(amount: i128) → Result<(), ErrorCode>` / `get_base_fee() → i128`

### `set_fee_admin(fee_admin: Address) → Result<(), ErrorCode>` / `get_fee_admin() → Option<Address>`

### `get_revenue(token: Address) → i128`

### `withdraw_protocol_fees(token: Address, recipient: Address) → Result<i128, ErrorCode>`

### `claim_referral_rewards(address: Address, token: Address) → Result<i128, ErrorCode>`

---

## Circuit Breaker

### `set_circuit_breaker(state: CircuitBreakerState) → Result<(), ErrorCode>`

States: `Closed | Open | HalfOpen | Paused`

### `pause() → Result<(), ErrorCode>` / `unpause() → Result<(), ErrorCode>`

### `reset_monitoring() → Result<(), ErrorCode>`

Admin-only. Clears error counters.

---

## Queries (Paginated)

All paginated queries silently clamp `limit` to **100** (`MAX_PAGE_LIMIT`). Callers requesting more receive at most 100 records — no error is returned.

### `get_markets(offset: u32, limit: u32) → Vec<Market>`

Returns all markets regardless of status, ordered by creation (ascending).

### `get_markets_by_status(status: MarketStatus, offset: u32, limit: u32) → Vec<Market>`

Filters by `Active | PendingResolution | Disputed | Resolved | Cancelled`. Iterates newest-first for fresher results.

### `get_guardians_paginated(offset: u32, limit: u32) → Vec<Guardian>`

### `get_admin() → Option<Address>`

---

## Error Codes

| Code | Value | Description |
|------|-------|-------------|
| `AlreadyInitialized` | 100 | Contract already initialized |
| `NotAuthorized` | 101 | Caller lacks required authorization |
| `MarketNotFound` | 102 | No market with the given ID |
| `MarketClosed` | 103 | Market deadline has passed |
| `MarketStillActive` | 104 | Market is still accepting bets |
| `InvalidOutcome` | 105 | Outcome index out of range |
| `InvalidBetAmount` | 106 | Bet amount is zero or below minimum |
| `InsufficientBalance` | 107 | Caller token balance too low |
| `OracleFailure` | 108 | Oracle cross-contract call failed |
| `CircuitBreakerOpen` | 109 | Circuit breaker is open; operation blocked |
| `DisputeWindowClosed` | 110 | Dispute window has expired |
| `VotingNotStarted` | 111 | Voting period has not begun |
| `VotingEnded` | 112 | Voting period has already ended |
| `AlreadyVoted` | 113 | Address has already cast a vote |
| `FeeTooHigh` | 114 | Proposed fee exceeds allowed maximum |
| `MarketNotActive` | 115 | Market is not in Active state |
| `DeadlinePassed` | 116 | Action attempted after deadline |
| `CannotChangeOutcome` | 117 | Outcome is already finalized |
| `MarketNotDisputed` | 118 | Market is not in Disputed state |
| `MarketNotPendingResolution` | 119 | Market is not in PendingResolution state |
| `AdminNotSet` | 120 | Admin account not configured |
| `ContractPaused` | 121 | Contract is paused via circuit breaker |
| `GuardianNotSet` | 122 | Guardian account not configured |
| `TooManyOutcomes` | 123 | Exceeds `MAX_OUTCOMES_PER_MARKET` (32) |
| `TooManyWinners` | 124 | Exceeds maximum push-payout winner threshold |
| `PayoutModeNotSupported` | 125 | Requested payout mode is not supported |
| `InsufficientDeposit` | 126 | Creation deposit not met |
| `TimelockActive` | 127 | Upgrade timelock has not elapsed |
| `UpgradeNotInitiated` | 128 | No pending upgrade to act on |
| `InsufficientVotes` | 129 | Not enough votes to proceed |
| `AlreadyVotedOnUpgrade` | 130 | Address already voted on this upgrade |
| `InvalidWasmHash` | 131 | Provided wasm hash is invalid |
| `UpgradeFailed` | 132 | Upgrade execution failed |
| `ParentMarketNotResolved` | 133 | Conditional market's parent is not yet resolved |
| `ParentMarketInvalidOutcome` | 134 | Parent market resolved to a different outcome |
| `ResolutionNotReady` | 135 | Conditions for resolution not yet met |
| `DisputeWindowStillOpen` | 136 | Dispute window has not yet closed |
| `NoMajorityReached` | 137 | No outcome reached the 60% majority threshold |
| `StalePrice` | 138 | Price feed `publish_time` older than `max_staleness_seconds` |
| `ConfidenceTooLow` | 139 | Oracle confidence interval exceeds `max_confidence_bps` |
| `InsufficientVotingWeight` | 140 | Voter's governance token balance too low |
| `MarketNotCancelled` | 141 | Market is not in Cancelled state |
| `BetNotFound` | 142 | No bet record for this bettor/market |
| `UpgradeAlreadyPending` | 143 | An upgrade proposal is already pending |
| `UpgradeHashInCooldown` | 144 | This wasm hash is in the 7-day cooldown period |
| `InvalidAmount` | 145 | Generic invalid amount |
| `GovernanceTokenNotSet` | 146 | Governance token address not configured |
| `MarketNotResolved` | 147 | Market has not been resolved yet |
| `InvalidDeadline` | 148 | Deadline is in the past or malformed |

---

## Events

All events follow the topic layout:
- **Topic 0:** Event name (short symbol, ≤ 9 chars)
- **Topic 1:** `market_id: u64` (primary indexer key; `0` for contract-level events)
- **Topic 2:** Triggering address

| Event | Topic Symbol | Topics | Data Payload |
|-------|-------------|--------|--------------|
| MarketCreated | `mkt_creat` | `(mkt_creat, market_id, creator)` | `(description: String, num_outcomes: u32, deadline: u64)` |
| BetPlaced | `bet_place` | `(bet_place, market_id, bettor)` | `(outcome: u32, amount: i128)` |
| DisputeFiled | `disp_file` | `(disp_file, market_id, disciplinarian)` | `new_deadline: u64` |
| ResolutionFinalized | `resolv_fx` | `(resolv_fx, market_id, resolver)` | `(winning_outcome: u32, total_payout: i128)` |
| RewardsClaimed | `reward_fx` | `(reward_fx, market_id, claimer)` | `(amount: i128, token_address: Address, is_refund: bool)` |
| VoteCast | `vote_cast` | `(vote_cast, market_id, voter)` | `(outcome: u32, weight: i128)` |
| CircuitBreakerTriggered | `cb_state` | `(cb_state, 0, contract_address)` | `state: String` |
| OracleResultSet | `oracle_ok` | `(oracle_ok, market_id, oracle_source)` | `(oracle_id: u32, outcome: u32)` |
| OracleResolved | `orcl_res` | `(orcl_res, market_id, oracle_address)` | `outcome: u32` |
| MarketFinalized | `mkt_final` | `(mkt_final, market_id, resolver)` | `winning_outcome: u32` |
| DisputeResolved | `disp_res` | `(disp_res, market_id, resolver)` | `winning_outcome: u32` |
| MarketCancelled (admin) | `mkt_cncl` | `(mkt_cncl, market_id, admin)` | `()` |
| MarketCancelledVote (community) | `mk_cn_vt` | `(mk_cn_vt, market_id, resolver)` | `()` |
| ReferralReward | `ref_rwrd` | `(ref_rwrd, market_id, referrer)` | `amount: i128` |
| ReferralClaimed | `ref_claim` | `(ref_claim, market_id, claimer)` | `amount: i128` |
| ReferralDistribution | `ref_dist` | `(ref_dist, market_id, token)` | `()` |
| CircuitBreakerAuto | `cb_auto` | `(cb_auto, 0, contract_address)` | `error_count: u32` |
| FeeCollected | `fee_colct` | `(fee_colct, 0, contract_address)` | `amount: i128` |
| AdminFallbackResolution | `adm_fbk` | `(adm_fbk, market_id, admin)` | `winning_outcome: u32` |
| CreatorReputationSet | `rep_set` | `(rep_set, creator)` | `(old_score: u32, new_score: u32)` |
| CreationDepositSet | `dep_set` | `(dep_set,)` | `(old_amount: i128, new_amount: i128)` |
| MonitoringStateReset | `mon_reset` | `(mon_reset, resetter)` | `(previous_error_count: u32, previous_last_observation: u64)` |
| MarketPruned | `mkt_prune` | `(mkt_prune, market_id)` | `pruned_at: u64` |
| UpgradeInitiated | `upg_init` | `(upg_init, initiator)` | `wasm_hash: BytesN<32>` |
| UpgradeVoted | `upg_vote` | `(upg_vote, voter)` | `vote_for: bool` |
| UpgradeExecuted | `upg_exec` | `(upg_exec, executor)` | `wasm_hash: BytesN<32>` |
| UpgradeRejected | `upg_rej` | `(upg_rej,)` | `wasm_hash: BytesN<32>` |
| MarketStateChanged | `mkt_state` | `(mkt_state, market_id)` | `(old_status: String, new_status: String, timestamp: u64)` |

> **Notes:**
> - `CircuitBreakerTriggered`, `CircuitBreakerAuto`, and `FeeCollected` use `market_id = 0` and the contract address as Topic 2.
> - `CreatorReputationSet` uses `(symbol, creator)` with no `market_id`.
> - `CreationDepositSet` uses `(symbol,)` only.
> - `MonitoringStateReset` uses `(symbol, resetter)` with no `market_id`.
> - `OracleResultSet` data includes `oracle_id` to identify which oracle source reported the result (multi-oracle support).
