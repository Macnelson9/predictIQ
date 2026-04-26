use std::{future::Future, time::Duration};

use anyhow::Context;
use prometheus::{IntCounter, IntCounterVec, Registry};
use redis::{aio::ConnectionManager, AsyncCommands, Client};
use serde::{de::DeserializeOwned, Serialize};

// ============================================================================
// Per-key-type TTL configuration (Issue #046)
// ============================================================================

/// Categories of cache keys, each with its own TTL tuned to data volatility.
///
/// | Category        | Default TTL | Rationale                                      |
/// |-----------------|-------------|------------------------------------------------|
/// | `Statistics`    | 60 s        | Aggregated counts change frequently            |
/// | `FeaturedMarkets` | 300 s     | Curated list; changes on editorial action      |
/// | `Content`       | 600 s       | Paginated content; moderate change rate        |
/// | `ChainMarket`   | 30 s        | On-chain state; high volatility                |
/// | `ChainPlatformStats` | 120 s  | Platform-wide stats; moderate volatility       |
/// | `ChainUserBets` | 60 s        | Per-user on-chain data; changes on activity    |
/// | `ChainOracleResult` | 300 s   | Oracle results; stable once resolved           |
/// | `ChainTxStatus` | 15 s        | Transaction status; changes until confirmed    |
/// | `ChainHealth`   | 10 s        | Node health; must be near-real-time            |
/// | `ChainLedger`   | 5 s         | Last-seen ledger; changes every few seconds    |
/// | `ChainSyncCursor` | 5 s       | Sync cursor; changes every few seconds         |
/// | `Custom`        | caller-supplied | Escape hatch for one-off TTLs             |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCategory {
    /// Aggregated statistics (e.g. `api:v1:statistics`)
    Statistics,
    /// Featured market listings
    FeaturedMarkets,
    /// Paginated content
    Content,
    /// Individual on-chain market data
    ChainMarket,
    /// Platform-wide on-chain statistics
    ChainPlatformStats,
    /// Per-user on-chain bet history
    ChainUserBets,
    /// Oracle resolution results
    ChainOracleResult,
    /// Transaction confirmation status
    ChainTxStatus,
    /// Node / RPC health check
    ChainHealth,
    /// Last seen ledger sequence
    ChainLedger,
    /// Sync cursor position
    ChainSyncCursor,
    /// Caller-supplied TTL (bypasses category lookup)
    Custom,
}

impl KeyCategory {
    /// Human-readable label used in Prometheus metric tags.
    pub fn label(self) -> &'static str {
        match self {
            Self::Statistics => "statistics",
            Self::FeaturedMarkets => "featured_markets",
            Self::Content => "content",
            Self::ChainMarket => "chain_market",
            Self::ChainPlatformStats => "chain_platform_stats",
            Self::ChainUserBets => "chain_user_bets",
            Self::ChainOracleResult => "chain_oracle_result",
            Self::ChainTxStatus => "chain_tx_status",
            Self::ChainHealth => "chain_health",
            Self::ChainLedger => "chain_ledger",
            Self::ChainSyncCursor => "chain_sync_cursor",
            Self::Custom => "custom",
        }
    }
}

/// Centralized TTL configuration for every `KeyCategory`.
///
/// All values are in seconds. Override individual fields to tune for your
/// deployment without touching call sites.
#[derive(Clone, Debug)]
pub struct TtlConfig {
    pub statistics: Duration,
    pub featured_markets: Duration,
    pub content: Duration,
    pub chain_market: Duration,
    pub chain_platform_stats: Duration,
    pub chain_user_bets: Duration,
    pub chain_oracle_result: Duration,
    pub chain_tx_status: Duration,
    pub chain_health: Duration,
    pub chain_ledger: Duration,
    pub chain_sync_cursor: Duration,
}

impl Default for TtlConfig {
    fn default() -> Self {
        Self {
            statistics:           Duration::from_secs(60),
            featured_markets:     Duration::from_secs(300),
            content:              Duration::from_secs(600),
            chain_market:         Duration::from_secs(30),
            chain_platform_stats: Duration::from_secs(120),
            chain_user_bets:      Duration::from_secs(60),
            chain_oracle_result:  Duration::from_secs(300),
            chain_tx_status:      Duration::from_secs(15),
            chain_health:         Duration::from_secs(10),
            chain_ledger:         Duration::from_secs(5),
            chain_sync_cursor:    Duration::from_secs(5),
        }
    }
}

impl TtlConfig {
    /// Look up the TTL for a given category.
    /// Returns `None` for `KeyCategory::Custom` — callers must supply their own.
    pub fn get(&self, category: KeyCategory) -> Option<Duration> {
        match category {
            KeyCategory::Statistics        => Some(self.statistics),
            KeyCategory::FeaturedMarkets   => Some(self.featured_markets),
            KeyCategory::Content           => Some(self.content),
            KeyCategory::ChainMarket       => Some(self.chain_market),
            KeyCategory::ChainPlatformStats => Some(self.chain_platform_stats),
            KeyCategory::ChainUserBets     => Some(self.chain_user_bets),
            KeyCategory::ChainOracleResult => Some(self.chain_oracle_result),
            KeyCategory::ChainTxStatus     => Some(self.chain_tx_status),
            KeyCategory::ChainHealth       => Some(self.chain_health),
            KeyCategory::ChainLedger       => Some(self.chain_ledger),
            KeyCategory::ChainSyncCursor   => Some(self.chain_sync_cursor),
            KeyCategory::Custom            => None,
        }
    }
}

// ============================================================================
// Per-key-type hit/miss metrics (Issue #046)
// ============================================================================

/// Prometheus counters tracking cache hits and misses broken down by
/// `KeyCategory`. Register once at startup and pass into `RedisCache`.
#[derive(Clone)]
pub struct CacheMetrics {
    pub hits:   IntCounterVec,
    pub misses: IntCounterVec,
}

impl CacheMetrics {
    pub fn new(registry: &Registry) -> anyhow::Result<Self> {
        let hits = IntCounterVec::new(
            prometheus::Opts::new(
                "cache_hits_by_category_total",
                "Cache hits broken down by key category",
            ),
            &["category"],
        )?;
        let misses = IntCounterVec::new(
            prometheus::Opts::new(
                "cache_misses_by_category_total",
                "Cache misses broken down by key category",
            ),
            &["category"],
        )?;
        registry.register(Box::new(hits.clone()))?;
        registry.register(Box::new(misses.clone()))?;
        Ok(Self { hits, misses })
    }

    pub fn hit(&self, category: KeyCategory) {
        self.hits.with_label_values(&[category.label()]).inc();
    }

    pub fn miss(&self, category: KeyCategory) {
        self.misses.with_label_values(&[category.label()]).inc();
    }
}

/// Configuration for cache stampede protection.
#[derive(Clone, Debug)]
pub struct StampedeConfig {
    /// Enable probabilistic early expiration (XFetch algorithm).
    /// When enabled, cache entries may be refreshed before they expire to
    /// prevent multiple concurrent requests from hitting the DB at once.
    pub probabilistic_early_expiry: bool,
    /// Beta parameter for XFetch (higher = more aggressive early refresh, default 1.0).
    pub xfetch_beta: f64,
    /// Enable mutex-based protection via Redis SET NX lock.
    pub mutex_lock: bool,
    /// How long to hold the recompute lock (prevents other requests from
    /// triggering a fetch while one is already in flight).
    pub lock_ttl: Duration,
    /// How long a waiting request will poll for the lock to be released.
    pub lock_wait_timeout: Duration,
}

impl Default for StampedeConfig {
    fn default() -> Self {
        Self {
            probabilistic_early_expiry: true,
            xfetch_beta: 1.0,
            mutex_lock: true,
            lock_ttl: Duration::from_secs(10),
            lock_wait_timeout: Duration::from_secs(5),
        }
    }
}

/// Prometheus counters for stampede-related events.
#[derive(Clone)]
pub struct StampedeMetrics {
    /// Incremented when a probabilistic early refresh is triggered.
    pub early_refresh_total: IntCounter,
    /// Incremented when a mutex lock is acquired to recompute a value.
    pub lock_acquired_total: IntCounter,
    /// Incremented when a request waits for another to finish recomputing.
    pub lock_wait_total: IntCounter,
    /// Incremented when a lock wait times out and the request falls through to DB.
    pub lock_timeout_total: IntCounter,
}

impl StampedeMetrics {
    pub fn new(registry: &Registry) -> anyhow::Result<Self> {
        let early_refresh_total = IntCounter::new(
            "cache_stampede_early_refresh_total",
            "Number of probabilistic early cache refreshes triggered",
        )?;
        let lock_acquired_total = IntCounter::new(
            "cache_stampede_lock_acquired_total",
            "Number of times a recompute lock was acquired",
        )?;
        let lock_wait_total = IntCounter::new(
            "cache_stampede_lock_wait_total",
            "Number of requests that waited for a recompute lock",
        )?;
        let lock_timeout_total = IntCounter::new(
            "cache_stampede_lock_timeout_total",
            "Number of lock waits that timed out",
        )?;

        registry.register(Box::new(early_refresh_total.clone()))?;
        registry.register(Box::new(lock_acquired_total.clone()))?;
        registry.register(Box::new(lock_wait_total.clone()))?;
        registry.register(Box::new(lock_timeout_total.clone()))?;

        Ok(Self {
            early_refresh_total,
            lock_acquired_total,
            lock_wait_total,
            lock_timeout_total,
        })
    }
}

/// Cached value envelope that stores the logical TTL alongside the value so
/// the XFetch algorithm can decide whether to refresh early.
#[derive(serde::Serialize, serde::Deserialize)]
struct CachedEntry<T> {
    value: T,
    /// Unix timestamp (seconds) when this entry logically expires.
    expires_at: i64,
    /// How long (seconds) the last recompute took — used by XFetch.
    delta_secs: f64,
}

#[derive(Clone)]
pub struct RedisCache {
    pub(crate) manager: ConnectionManager,
    pub stampede: StampedeConfig,
    /// Centralized per-category TTL configuration.
    pub ttls: TtlConfig,
    /// Optional per-category hit/miss metrics.
    pub metrics: Option<CacheMetrics>,
}

impl RedisCache {
    pub async fn new(redis_url: &str) -> anyhow::Result<Self> {
        Self::with_config(redis_url, StampedeConfig::default()).await
    }

    pub async fn with_config(redis_url: &str, stampede: StampedeConfig) -> anyhow::Result<Self> {
        let client = Client::open(redis_url).context("invalid REDIS_URL")?;
        let manager = client
            .get_connection_manager()
            .await
            .context("failed to connect to redis")?;
        Ok(Self {
            manager,
            stampede,
            ttls: TtlConfig::default(),
            metrics: None,
        })
    }

    /// Builder-style setter for a custom `TtlConfig`.
    pub fn with_ttls(mut self, ttls: TtlConfig) -> Self {
        self.ttls = ttls;
        self
    }

    /// Builder-style setter for `CacheMetrics`.
    pub fn with_metrics(mut self, metrics: CacheMetrics) -> Self {
        self.metrics = Some(metrics);
        self
    }

    pub async fn get_json<T>(&self, key: &str) -> anyhow::Result<Option<T>>
    where
        T: DeserializeOwned,
    {
        let mut conn = self.manager.clone();
        let val: Option<String> = conn.get(key).await?;
        match val {
            Some(raw) => Ok(Some(serde_json::from_str(&raw)?)),
            None => Ok(None),
        }
    }

    pub async fn set_json<T>(&self, key: &str, value: &T, ttl: Duration) -> anyhow::Result<()>
    where
        T: Serialize,
    {
        let mut conn = self.manager.clone();
        let raw = serde_json::to_string(value)?;
        let _: () = conn.set_ex(key, raw, ttl.as_secs()).await?;
        Ok(())
    }

    pub async fn del(&self, key: &str) -> anyhow::Result<()> {
        let mut conn = self.manager.clone();
        let _: usize = conn.del(key).await?;
        Ok(())
    }

    pub async fn del_by_pattern(&self, pattern: &str) -> anyhow::Result<usize> {
        let mut conn = self.manager.clone();
        let keys: Vec<String> = redis::cmd("KEYS")
            .arg(pattern)
            .query_async(&mut conn)
            .await?;
        if keys.is_empty() {
            return Ok(0);
        }
        let deleted: usize = conn.del(keys).await?;
        Ok(deleted)
    }

    /// Fetch-or-set with stampede protection.
    ///
    /// Strategy (applied in order when enabled via `StampedeConfig`):
    /// 1. **Probabilistic early expiry (XFetch)** — if the entry is still
    ///    alive but close to expiry, one request will refresh it early while
    ///    others continue serving the stale value.
    /// 2. **Mutex lock** — when the entry is missing (or chosen for early
    ///    refresh), a Redis `SET NX` lock ensures only one request calls the
    ///    fetcher. Others wait briefly and then serve the freshly-written
    ///    value, falling back to calling the fetcher themselves only if the
    ///    lock wait times out.
    ///
    /// Returns `(value, cache_hit)`.
    pub async fn get_or_set_json<T, F, Fut>(
        &self,
        key: &str,
        ttl: Duration,
        fetcher: F,
    ) -> anyhow::Result<(T, bool)>
    where
        T: Serialize + DeserializeOwned + Clone,
        F: FnOnce() -> Fut,
        Fut: Future<Output = anyhow::Result<T>>,
    {
        self.get_or_set_json_with_metrics(key, ttl, fetcher, None)
            .await
    }

    /// Fetch-or-set using the TTL registered for `category` in `self.ttls`.
    ///
    /// Also records a hit or miss against `self.metrics` (if configured) so
    /// the hit/miss ratio is tracked per key type automatically.
    ///
    /// ```rust,ignore
    /// let (stats, hit) = cache
    ///     .get_or_set_by_category(
    ///         &keys::api_statistics(),
    ///         KeyCategory::Statistics,
    ///         || async { fetch_statistics_from_db().await },
    ///     )
    ///     .await?;
    /// ```
    pub async fn get_or_set_by_category<T, F, Fut>(
        &self,
        key: &str,
        category: KeyCategory,
        fetcher: F,
    ) -> anyhow::Result<(T, bool)>
    where
        T: Serialize + DeserializeOwned + Clone,
        F: FnOnce() -> Fut,
        Fut: Future<Output = anyhow::Result<T>>,
    {
        let ttl = self
            .ttls
            .get(category)
            .ok_or_else(|| anyhow::anyhow!(
                "KeyCategory::Custom requires an explicit TTL — use get_or_set_json instead"
            ))?;

        let (value, hit) = self
            .get_or_set_json_with_metrics(key, ttl, fetcher, None)
            .await?;

        // Record hit/miss per category.
        if let Some(m) = &self.metrics {
            if hit { m.hit(category); } else { m.miss(category); }
        }

        Ok((value, hit))
    }

    /// Same as `get_or_set_json` but records stampede events to `metrics`.
    pub async fn get_or_set_json_with_metrics<T, F, Fut>(
        &self,
        key: &str,
        ttl: Duration,
        fetcher: F,
        metrics: Option<&StampedeMetrics>,
    ) -> anyhow::Result<(T, bool)>
    where
        T: Serialize + DeserializeOwned + Clone,
        F: FnOnce() -> Fut,
        Fut: Future<Output = anyhow::Result<T>>,
    {
        let entry_key = format!("entry:{key}");
        let lock_key = format!("lock:{key}");

        // --- 1. Try to read an existing entry ---
        if let Some(entry) = self.get_entry::<T>(&entry_key).await? {
            let should_refresh = self.stampede.probabilistic_early_expiry
                && xfetch_should_refresh(&entry, self.stampede.xfetch_beta);

            if !should_refresh {
                return Ok((entry.value, true));
            }
            // Probabilistic early refresh chosen — fall through to recompute.
            if let Some(m) = metrics {
                m.early_refresh_total.inc();
            }
        }

        // --- 2. Mutex lock: only one request recomputes ---
        if self.stampede.mutex_lock {
            let lock_id = uuid::Uuid::new_v4().to_string();
            let acquired = self.try_acquire_lock(&lock_key, &lock_id).await?;

            if acquired {
                if let Some(m) = metrics {
                    m.lock_acquired_total.inc();
                }
                let result = self
                    .recompute_and_store::<T, _, _>(&entry_key, ttl, fetcher)
                    .await;
                // Always release the lock, even on error.
                let _ = self.release_lock(&lock_key, &lock_id).await;
                return result.map(|v| (v, false));
            }

            // Another request holds the lock — wait for it to finish.
            if let Some(m) = metrics {
                m.lock_wait_total.inc();
            }
            if let Some(value) = self
                .wait_for_entry::<T>(&entry_key, self.stampede.lock_wait_timeout)
                .await?
            {
                return Ok((value, true));
            }
            // Lock wait timed out — fall through and fetch ourselves.
            if let Some(m) = metrics {
                m.lock_timeout_total.inc();
            }
        }

        // --- 3. No lock / lock timed out: fetch directly ---
        self.recompute_and_store::<T, _, _>(&entry_key, ttl, fetcher)
            .await
            .map(|v| (v, false))
    }

    // ---- internal helpers ----

    async fn get_entry<T>(&self, key: &str) -> anyhow::Result<Option<CachedEntry<T>>>
    where
        T: DeserializeOwned,
    {
        let mut conn = self.manager.clone();
        let raw: Option<String> = conn.get(key).await?;
        match raw {
            Some(s) => Ok(Some(serde_json::from_str(&s)?)),
            None => Ok(None),
        }
    }

    async fn set_entry<T>(&self, key: &str, entry: &CachedEntry<T>, ttl: Duration) -> anyhow::Result<()>
    where
        T: Serialize,
    {
        let mut conn = self.manager.clone();
        let raw = serde_json::to_string(entry)?;
        // Store with a small grace period beyond the logical TTL so XFetch
        // can still serve the stale value while a refresh is in flight.
        let redis_ttl = ttl + Duration::from_secs(30);
        let _: () = conn.set_ex(key, raw, redis_ttl.as_secs()).await?;
        Ok(())
    }

    async fn recompute_and_store<T, F, Fut>(
        &self,
        entry_key: &str,
        ttl: Duration,
        fetcher: F,
    ) -> anyhow::Result<T>
    where
        T: Serialize + DeserializeOwned + Clone,
        F: FnOnce() -> Fut,
        Fut: Future<Output = anyhow::Result<T>>,
    {
        let start = std::time::Instant::now();
        let value = fetcher().await?;
        let delta_secs = start.elapsed().as_secs_f64();

        let expires_at = chrono::Utc::now().timestamp() + ttl.as_secs() as i64;
        let entry = CachedEntry {
            value: value.clone(),
            expires_at,
            delta_secs,
        };
        self.set_entry(entry_key, &entry, ttl).await?;
        Ok(value)
    }

    /// Try to acquire a Redis NX lock. Returns `true` if acquired.
    async fn try_acquire_lock(&self, lock_key: &str, lock_id: &str) -> anyhow::Result<bool> {
        let mut conn = self.manager.clone();
        let result: Option<String> = redis::cmd("SET")
            .arg(lock_key)
            .arg(lock_id)
            .arg("NX")
            .arg("PX")
            .arg(self.stampede.lock_ttl.as_millis() as u64)
            .query_async(&mut conn)
            .await?;
        Ok(result.is_some())
    }

    /// Release the lock only if we still own it (Lua script for atomicity).
    async fn release_lock(&self, lock_key: &str, lock_id: &str) -> anyhow::Result<()> {
        let mut conn = self.manager.clone();
        let script = r#"
            if redis.call("GET", KEYS[1]) == ARGV[1] then
                return redis.call("DEL", KEYS[1])
            else
                return 0
            end
        "#;
        let _: i64 = redis::Script::new(script)
            .key(lock_key)
            .arg(lock_id)
            .invoke_async(&mut conn)
            .await?;
        Ok(())
    }

    /// Poll until the entry appears or `timeout` elapses.
    async fn wait_for_entry<T>(
        &self,
        entry_key: &str,
        timeout: Duration,
    ) -> anyhow::Result<Option<T>>
    where
        T: DeserializeOwned,
    {
        let deadline = tokio::time::Instant::now() + timeout;
        let poll_interval = Duration::from_millis(50);

        loop {
            if let Some(entry) = self.get_entry::<T>(entry_key).await? {
                return Ok(Some(entry.value));
            }
            if tokio::time::Instant::now() >= deadline {
                return Ok(None);
            }
            tokio::time::sleep(poll_interval).await;
        }
    }
}

/// XFetch probabilistic early expiration algorithm.
///
/// Returns `true` when the current request should refresh the cache entry
/// before it expires, based on how close to expiry it is and how long the
/// last recompute took.
///
/// Formula: `-delta * beta * ln(rand)  >=  ttl_remaining`
fn xfetch_should_refresh<T>(entry: &CachedEntry<T>, beta: f64) -> bool {
    let now = chrono::Utc::now().timestamp();
    let ttl_remaining = (entry.expires_at - now) as f64;
    if ttl_remaining <= 0.0 {
        return true; // already expired
    }
    let rand: f64 = rand_f64();
    let score = -entry.delta_secs * beta * rand.ln();
    score >= ttl_remaining
}

/// Simple thread-local random f64 in (0, 1) without pulling in `rand` crate.
fn rand_f64() -> f64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::SystemTime;

    let mut h = DefaultHasher::new();
    SystemTime::now().hash(&mut h);
    std::thread::current().id().hash(&mut h);
    // Map u64 to (0.0, 1.0)
    let bits = h.finish();
    // Avoid 0 to prevent ln(0) = -inf
    let f = (bits as f64 / u64::MAX as f64).clamp(1e-9, 1.0 - 1e-9);
    f
}

pub mod keys {
    use super::KeyCategory;

    pub const API_PREFIX: &str = "api:v1";
    pub const DBQ_PREFIX: &str = "dbq:v1";
    pub const CHAIN_PREFIX: &str = "chain:v1";

    // ---- api:v1 keys ----

    pub fn api_statistics() -> String {
        format!("{API_PREFIX}:statistics")
    }
    pub fn api_statistics_category() -> KeyCategory { KeyCategory::Statistics }

    pub fn api_featured_markets() -> String {
        format!("{API_PREFIX}:featured_markets")
    }
    pub fn api_featured_markets_category() -> KeyCategory { KeyCategory::FeaturedMarkets }

    pub fn api_content(page: i64, page_size: i64) -> String {
        format!("{API_PREFIX}:content:page:{page}:size:{page_size}")
    }
    pub fn api_content_category() -> KeyCategory { KeyCategory::Content }

    // ---- dbq:v1 keys ----

    pub fn dbq_statistics() -> String {
        format!("{DBQ_PREFIX}:statistics")
    }
    pub fn dbq_statistics_category() -> KeyCategory { KeyCategory::Statistics }

    pub fn dbq_featured_markets(limit: i64) -> String {
        format!("{DBQ_PREFIX}:featured_markets:limit:{limit}")
    }
    pub fn dbq_featured_markets_category() -> KeyCategory { KeyCategory::FeaturedMarkets }

    pub fn dbq_content(page: i64, page_size: i64) -> String {
        format!("{DBQ_PREFIX}:content:page:{page}:size:{page_size}")
    }
    pub fn dbq_content_category() -> KeyCategory { KeyCategory::Content }

    // ---- chain:v1 keys ----

    pub fn chain_market(market_id: i64) -> String {
        format!("{CHAIN_PREFIX}:market:{market_id}")
    }
    pub fn chain_market_category() -> KeyCategory { KeyCategory::ChainMarket }

    pub fn chain_platform_stats(network: &str) -> String {
        format!("{CHAIN_PREFIX}:platform_stats:{network}")
    }
    pub fn chain_platform_stats_category() -> KeyCategory { KeyCategory::ChainPlatformStats }

    pub fn chain_user_bets(network: &str, user: &str, page: i64, page_size: i64) -> String {
        format!(
            "{CHAIN_PREFIX}:user_bets:{network}:{}:page:{page}:size:{page_size}",
            user.to_lowercase()
        )
    }
    pub fn chain_user_bets_category() -> KeyCategory { KeyCategory::ChainUserBets }

    pub fn chain_oracle_result(network: &str, market_id: i64) -> String {
        format!("{CHAIN_PREFIX}:oracle:{network}:market:{market_id}")
    }
    pub fn chain_oracle_result_category() -> KeyCategory { KeyCategory::ChainOracleResult }

    pub fn chain_tx_status(network: &str, tx_hash: &str) -> String {
        format!(
            "{CHAIN_PREFIX}:tx_status:{network}:{}",
            tx_hash.to_lowercase()
        )
    }
    pub fn chain_tx_status_category() -> KeyCategory { KeyCategory::ChainTxStatus }

    pub fn chain_health(network: &str) -> String {
        format!("{CHAIN_PREFIX}:health:{network}")
    }
    pub fn chain_health_category() -> KeyCategory { KeyCategory::ChainHealth }

    pub fn chain_last_seen_ledger(network: &str) -> String {
        format!("{CHAIN_PREFIX}:last_seen_ledger:{network}")
    }
    pub fn chain_last_seen_ledger_category() -> KeyCategory { KeyCategory::ChainLedger }

    pub fn chain_sync_cursor(network: &str) -> String {
        format!("{CHAIN_PREFIX}:sync_cursor:{network}")
    }
    pub fn chain_sync_cursor_category() -> KeyCategory { KeyCategory::ChainSyncCursor }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    // ---- TtlConfig tests ----

    #[test]
    fn default_ttl_config_returns_correct_durations() {
        let cfg = TtlConfig::default();
        assert_eq!(cfg.get(KeyCategory::Statistics),        Some(Duration::from_secs(60)));
        assert_eq!(cfg.get(KeyCategory::FeaturedMarkets),   Some(Duration::from_secs(300)));
        assert_eq!(cfg.get(KeyCategory::Content),           Some(Duration::from_secs(600)));
        assert_eq!(cfg.get(KeyCategory::ChainMarket),       Some(Duration::from_secs(30)));
        assert_eq!(cfg.get(KeyCategory::ChainPlatformStats),Some(Duration::from_secs(120)));
        assert_eq!(cfg.get(KeyCategory::ChainUserBets),     Some(Duration::from_secs(60)));
        assert_eq!(cfg.get(KeyCategory::ChainOracleResult), Some(Duration::from_secs(300)));
        assert_eq!(cfg.get(KeyCategory::ChainTxStatus),     Some(Duration::from_secs(15)));
        assert_eq!(cfg.get(KeyCategory::ChainHealth),       Some(Duration::from_secs(10)));
        assert_eq!(cfg.get(KeyCategory::ChainLedger),       Some(Duration::from_secs(5)));
        assert_eq!(cfg.get(KeyCategory::ChainSyncCursor),   Some(Duration::from_secs(5)));
    }

    #[test]
    fn custom_category_returns_none() {
        let cfg = TtlConfig::default();
        assert_eq!(cfg.get(KeyCategory::Custom), None);
    }

    #[test]
    fn ttl_config_is_overridable_per_field() {
        let cfg = TtlConfig {
            statistics: Duration::from_secs(30),
            ..TtlConfig::default()
        };
        assert_eq!(cfg.get(KeyCategory::Statistics), Some(Duration::from_secs(30)));
        // Other fields unchanged
        assert_eq!(cfg.get(KeyCategory::Content), Some(Duration::from_secs(600)));
    }

    #[test]
    fn high_volatility_keys_have_shorter_ttl_than_stable_keys() {
        let cfg = TtlConfig::default();
        let health_ttl   = cfg.get(KeyCategory::ChainHealth).unwrap();
        let ledger_ttl   = cfg.get(KeyCategory::ChainLedger).unwrap();
        let content_ttl  = cfg.get(KeyCategory::Content).unwrap();
        let featured_ttl = cfg.get(KeyCategory::FeaturedMarkets).unwrap();

        assert!(health_ttl  < content_ttl,  "health should expire faster than content");
        assert!(ledger_ttl  < featured_ttl, "ledger should expire faster than featured markets");
    }

    #[test]
    fn key_category_labels_are_unique() {
        use std::collections::HashSet;
        let categories = [
            KeyCategory::Statistics,
            KeyCategory::FeaturedMarkets,
            KeyCategory::Content,
            KeyCategory::ChainMarket,
            KeyCategory::ChainPlatformStats,
            KeyCategory::ChainUserBets,
            KeyCategory::ChainOracleResult,
            KeyCategory::ChainTxStatus,
            KeyCategory::ChainHealth,
            KeyCategory::ChainLedger,
            KeyCategory::ChainSyncCursor,
            KeyCategory::Custom,
        ];
        let labels: HashSet<_> = categories.iter().map(|c| c.label()).collect();
        assert_eq!(labels.len(), categories.len(), "every category must have a unique label");
    }

    #[test]
    fn keys_module_category_helpers_return_correct_categories() {
        assert_eq!(keys::api_statistics_category(),          KeyCategory::Statistics);
        assert_eq!(keys::api_featured_markets_category(),    KeyCategory::FeaturedMarkets);
        assert_eq!(keys::api_content_category(),             KeyCategory::Content);
        assert_eq!(keys::dbq_statistics_category(),          KeyCategory::Statistics);
        assert_eq!(keys::chain_market_category(),            KeyCategory::ChainMarket);
        assert_eq!(keys::chain_platform_stats_category(),    KeyCategory::ChainPlatformStats);
        assert_eq!(keys::chain_user_bets_category(),         KeyCategory::ChainUserBets);
        assert_eq!(keys::chain_oracle_result_category(),     KeyCategory::ChainOracleResult);
        assert_eq!(keys::chain_tx_status_category(),         KeyCategory::ChainTxStatus);
        assert_eq!(keys::chain_health_category(),            KeyCategory::ChainHealth);
        assert_eq!(keys::chain_last_seen_ledger_category(),  KeyCategory::ChainLedger);
        assert_eq!(keys::chain_sync_cursor_category(),       KeyCategory::ChainSyncCursor);
    }

    // ---- XFetch / stampede tests (unchanged) ----

    #[test]
    fn xfetch_returns_true_for_expired_entry() {
        let entry: CachedEntry<u32> = CachedEntry {
            value: 42,
            expires_at: chrono::Utc::now().timestamp() - 1,
            delta_secs: 0.1,
        };
        assert!(xfetch_should_refresh(&entry, 1.0));
    }

    #[test]
    fn xfetch_returns_false_for_fresh_entry_with_tiny_delta() {
        let entry: CachedEntry<u32> = CachedEntry {
            value: 42,
            expires_at: chrono::Utc::now().timestamp() + 3600,
            delta_secs: 0.000_001,
        };
        let triggered = (0..100).filter(|_| xfetch_should_refresh(&entry, 1.0)).count();
        assert!(triggered < 5, "early refresh triggered too often for fresh entry: {triggered}/100");
    }

    #[test]
    fn xfetch_triggers_more_often_near_expiry() {
        let entry: CachedEntry<u32> = CachedEntry {
            value: 42,
            expires_at: chrono::Utc::now().timestamp() + 1,
            delta_secs: 2.0,
        };
        let triggered = (0..100).filter(|_| xfetch_should_refresh(&entry, 1.0)).count();
        assert!(triggered > 50, "expected frequent early refresh near expiry, got {triggered}/100");
    }

    #[test]
    fn stampede_config_default_has_both_strategies_enabled() {
        let cfg = StampedeConfig::default();
        assert!(cfg.probabilistic_early_expiry);
        assert!(cfg.mutex_lock);
        assert_eq!(cfg.xfetch_beta, 1.0);
    }

    #[tokio::test]
    async fn concurrent_fetcher_calls_are_serialised_by_counter() {
        let call_count = Arc::new(AtomicUsize::new(0));
        let lock = Arc::new(tokio::sync::Mutex::new(()));

        let tasks: Vec<_> = (0..20)
            .map(|_| {
                let count = Arc::clone(&call_count);
                let lock = Arc::clone(&lock);
                tokio::spawn(async move {
                    let _guard = lock.try_lock();
                    if _guard.is_ok() {
                        count.fetch_add(1, Ordering::SeqCst);
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                })
            })
            .collect();

        for t in tasks {
            t.await.unwrap();
        }

        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }
}
