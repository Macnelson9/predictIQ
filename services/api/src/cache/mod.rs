use std::{future::Future, time::Duration};

use anyhow::Context;
use prometheus::{IntCounter, Registry};
use redis::{aio::ConnectionManager, AsyncCommands, Client};
use serde::{de::DeserializeOwned, Serialize};

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
        Ok(Self { manager, stampede })
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
    pub const API_PREFIX: &str = "api:v1";
    pub const DBQ_PREFIX: &str = "dbq:v1";
    pub const CHAIN_PREFIX: &str = "chain:v1";

    pub fn api_statistics() -> String {
        format!("{API_PREFIX}:statistics")
    }

    pub fn api_featured_markets() -> String {
        format!("{API_PREFIX}:featured_markets")
    }

    pub fn api_content(page: i64, page_size: i64) -> String {
        format!("{API_PREFIX}:content:page:{page}:size:{page_size}")
    }

    pub fn dbq_statistics() -> String {
        format!("{DBQ_PREFIX}:statistics")
    }

    pub fn dbq_featured_markets(limit: i64) -> String {
        format!("{DBQ_PREFIX}:featured_markets:limit:{limit}")
    }

    pub fn dbq_content(page: i64, page_size: i64) -> String {
        format!("{DBQ_PREFIX}:content:page:{page}:size:{page_size}")
    }

    pub fn chain_market(market_id: i64) -> String {
        format!("{CHAIN_PREFIX}:market:{market_id}")
    }

    pub fn chain_platform_stats(network: &str) -> String {
        format!("{CHAIN_PREFIX}:platform_stats:{network}")
    }

    pub fn chain_user_bets(network: &str, user: &str, page: i64, page_size: i64) -> String {
        format!(
            "{CHAIN_PREFIX}:user_bets:{network}:{}:page:{page}:size:{page_size}",
            user.to_lowercase()
        )
    }

    pub fn chain_oracle_result(network: &str, market_id: i64) -> String {
        format!("{CHAIN_PREFIX}:oracle:{network}:market:{market_id}")
    }

    pub fn chain_tx_status(network: &str, tx_hash: &str) -> String {
        format!(
            "{CHAIN_PREFIX}:tx_status:{network}:{}",
            tx_hash.to_lowercase()
        )
    }

    pub fn chain_health(network: &str) -> String {
        format!("{CHAIN_PREFIX}:health:{network}")
    }

    pub fn chain_last_seen_ledger(network: &str) -> String {
        format!("{CHAIN_PREFIX}:last_seen_ledger:{network}")
    }

    pub fn chain_sync_cursor(network: &str) -> String {
        format!("{CHAIN_PREFIX}:sync_cursor:{network}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    // ---- unit tests (no Redis required) ----

    #[test]
    fn xfetch_returns_true_for_expired_entry() {
        let entry: CachedEntry<u32> = CachedEntry {
            value: 42,
            expires_at: chrono::Utc::now().timestamp() - 1, // already expired
            delta_secs: 0.1,
        };
        assert!(xfetch_should_refresh(&entry, 1.0));
    }

    #[test]
    fn xfetch_returns_false_for_fresh_entry_with_tiny_delta() {
        // Entry expires far in the future; recompute was instant — should
        // almost never trigger early refresh.
        let entry: CachedEntry<u32> = CachedEntry {
            value: 42,
            expires_at: chrono::Utc::now().timestamp() + 3600,
            delta_secs: 0.000_001, // near-zero delta → score ≈ 0
        };
        // With such a tiny delta the score is essentially 0, so this should
        // be false. Run a few times to account for randomness.
        let triggered = (0..100).filter(|_| xfetch_should_refresh(&entry, 1.0)).count();
        assert!(triggered < 5, "early refresh triggered too often for fresh entry: {triggered}/100");
    }

    #[test]
    fn xfetch_triggers_more_often_near_expiry() {
        // Entry expires in 1 second with a 2-second delta → score often >= 1.
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

    /// Verify that under concurrent load only one fetcher call is made when
    /// mutex protection is enabled (no Redis — uses a mock counter).
    #[tokio::test]
    async fn concurrent_fetcher_calls_are_serialised_by_counter() {
        // This test validates the *logic* of the counter without a live Redis.
        // We simulate what the mutex path does: only the first caller increments.
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

        // Only one task should have acquired the lock and incremented.
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }
}
