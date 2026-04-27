# predictiq-api

Rust/Axum HTTP API service for the PredictIQ platform.

## Running

```bash
cargo run -p predictiq-api
```

## Running Tests

```bash
# All tests (unit + integration)
cargo test -p predictiq-api

# Integration tests only
cargo test -p predictiq-api --test integration_test

# Security / unit tests
cargo test -p predictiq-api --test security_tests

# Single-run (no watch mode)
cargo test -p predictiq-api -- --nocapture
```

## Environment Variables

| Variable | Default | Description |
|---|---|---|
| `API_BIND_ADDR` | `0.0.0.0:8080` | TCP address to listen on |
| `DATABASE_URL` | `postgres://postgres:postgres@127.0.0.1/predictiq` | PostgreSQL connection string |
| `REDIS_URL` | `redis://127.0.0.1:6379` | Redis connection string |
| `DB_POOL_MIN_CONNECTIONS` | `5` | Minimum pool connections |
| `DB_POOL_MAX_CONNECTIONS` | `25` | Maximum pool connections |
| `DB_POOL_ACQUIRE_TIMEOUT_SECS` | `5` | Seconds to wait for a free connection |
| `DB_POOL_IDLE_TIMEOUT_SECS` | _(sqlx default)_ | Seconds before idle connections are reaped |
| `DB_POOL_MAX_LIFETIME_SECS` | _(sqlx default)_ | Max lifetime of a connection |
| `DB_QUERY_TIMEOUT_SECS` | `30` | Per-query execution timeout |
| `BLOCKCHAIN_RPC_URL` | testnet default | Soroban RPC endpoint |
| `PREDICTIQ_CONTRACT_ID` | `predictiq_contract` | On-chain contract ID |
| `API_KEYS` | _(none)_ | Comma-separated admin API keys |
| `ADMIN_WHITELIST_IPS` | _(none)_ | Comma-separated IPs allowed to hit admin routes |
| `TRUST_PROXY` | `true` | Trust `X-Forwarded-For` header |
| `METRICS_PUBLIC` | `false` | Expose `/metrics` without auth |

See `DATABASE.md` for database-specific configuration.
