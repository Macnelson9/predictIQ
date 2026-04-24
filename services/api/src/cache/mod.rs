use std::{future::Future, time::Duration};

use anyhow::Context;
use redis::{aio::ConnectionManager, AsyncCommands, Client};
use serde::{de::DeserializeOwned, Serialize};

#[derive(Clone)]
pub struct RedisCache {
    pub(crate) manager: ConnectionManager,
}

impl RedisCache {
    pub async fn new(redis_url: &str) -> anyhow::Result<Self> {
        let client = Client::open(redis_url).context("invalid REDIS_URL")?;
        let manager = client
            .get_connection_manager()
            .await
            .context("failed to connect to redis")?;
        Ok(Self { manager })
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
        let mut cursor: u64 = 0;
        let mut total_deleted: usize = 0;
        loop {
            let (next_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(pattern)
                .arg("COUNT")
                .arg(100u64)
                .query_async(&mut conn)
                .await?;
            if !keys.is_empty() {
                let deleted: usize = conn.del(keys).await?;
                total_deleted += deleted;
            }
            cursor = next_cursor;
            if cursor == 0 {
                break;
            }
        }
        Ok(total_deleted)
    }

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
        if let Some(cached) = self.get_json(key).await? {
            return Ok((cached, true));
        }

        let value = fetcher().await?;
        self.set_json(key, &value, ttl).await?;
        Ok((value, false))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use testcontainers::runners::AsyncRunner;
    use testcontainers_modules::redis::Redis;

    use super::RedisCache;

    async fn start_cache() -> (RedisCache, impl Drop) {
        let container = Redis::default().start().await.expect("redis container");
        let port = container
            .get_host_port_ipv4(6379)
            .await
            .expect("redis port");
        let url = format!("redis://127.0.0.1:{port}");
        let cache = RedisCache::new(&url).await.expect("redis cache");
        (cache, container)
    }

    #[tokio::test]
    async fn cache_miss_populates_on_first_request() {
        let (cache, _c) = start_cache().await;
        let (val, hit) = cache
            .get_or_set_json::<u32, _, _>("key:miss", Duration::from_secs(60), || async {
                Ok(42u32)
            })
            .await
            .unwrap();
        assert_eq!(val, 42);
        assert!(!hit, "first call must be a miss");
        // Value must now be stored — a second call returns a hit with the same value.
        let (val2, hit2) = cache
            .get_or_set_json::<u32, _, _>("key:miss", Duration::from_secs(60), || async {
                Ok(0u32) // would overwrite if fetcher were called
            })
            .await
            .unwrap();
        assert_eq!(val2, 42, "stored value must be returned on hit");
        assert!(hit2, "second call must be a hit");
    }

    #[tokio::test]
    async fn cache_hit_on_subsequent_request() {
        let (cache, _c) = start_cache().await;
        cache
            .set_json("key:hit", &99u32, Duration::from_secs(60))
            .await
            .unwrap();
        let (val, hit) = cache
            .get_or_set_json::<u32, _, _>("key:hit", Duration::from_secs(60), || async {
                Ok(0u32) // must not be called
            })
            .await
            .unwrap();
        assert_eq!(val, 99, "cached value must be returned");
        assert!(hit, "pre-populated key must be a hit");
    }

    #[tokio::test]
    async fn del_invalidates_cached_entry() {
        let (cache, _c) = start_cache().await;
        cache
            .set_json("key:del", &7u32, Duration::from_secs(60))
            .await
            .unwrap();
        cache.del("key:del").await.unwrap();
        let result: Option<u32> = cache.get_json("key:del").await.unwrap();
        assert!(result.is_none(), "entry must be absent after del");
    }

    #[tokio::test]
    async fn del_by_pattern_invalidates_matching_entries() {
        let (cache, _c) = start_cache().await;
        for i in 0..3u32 {
            cache
                .set_json(&format!("ns:item:{i}"), &i, Duration::from_secs(60))
                .await
                .unwrap();
        }
        cache
            .set_json("other:item:0", &100u32, Duration::from_secs(60))
            .await
            .unwrap();

        let deleted = cache.del_by_pattern("ns:item:*").await.unwrap();
        assert_eq!(deleted, 3);

        for i in 0..3u32 {
            let v: Option<u32> = cache.get_json(&format!("ns:item:{i}")).await.unwrap();
            assert!(v.is_none(), "ns:item:{i} must be gone");
        }
        // unrelated key must survive
        let other: Option<u32> = cache.get_json("other:item:0").await.unwrap();
        assert_eq!(other, Some(100));
    }
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

    pub fn api_content(limit: i64) -> String {
        format!("{API_PREFIX}:content:limit:{limit}")
    }

    pub fn dbq_statistics() -> String {
        format!("{DBQ_PREFIX}:statistics")
    }

    pub fn dbq_featured_markets(limit: i64) -> String {
        format!("{DBQ_PREFIX}:featured_markets:limit:{limit}")
    }

    pub fn dbq_content(limit: i64) -> String {
        format!("{DBQ_PREFIX}:content:limit:{limit}")
    }

    pub fn chain_market(market_id: i64) -> String {
        format!("{CHAIN_PREFIX}:market:{market_id}")
    }

    pub fn chain_platform_stats(network: &str) -> String {
        format!("{CHAIN_PREFIX}:platform_stats:{network}")
    }

    pub fn chain_user_bets(network: &str, user: &str, limit: i64) -> String {
        format!(
            "{CHAIN_PREFIX}:user_bets:{network}:{}:limit:{limit}",
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

    pub fn chain_replay_progress(network: &str, from_ledger: u32) -> String {
        format!("{CHAIN_PREFIX}:replay:{network}:{from_ledger}")
    }
}
