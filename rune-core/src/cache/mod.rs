use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use dashmap::DashMap;
use tracing::{debug, trace};

use crate::search::{SearchQuery, SearchResponse};

/// Cache metrics for monitoring performance
#[derive(Debug, Default)]
pub struct CacheMetrics {
    pub l1_hits: std::sync::atomic::AtomicU64,
    pub l1_misses: std::sync::atomic::AtomicU64,
    pub total_queries: std::sync::atomic::AtomicU64,
    pub total_cache_time_us: std::sync::atomic::AtomicU64,
}

impl CacheMetrics {
    pub fn record_hit(&self) {
        self.l1_hits
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn record_miss(&self) {
        self.l1_misses
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn record_query(&self) {
        self.total_queries
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn record_cache_time(&self, us: u64) {
        self.total_cache_time_us
            .fetch_add(us, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn get_hit_rate(&self) -> f64 {
        let hits = self.l1_hits.load(std::sync::atomic::Ordering::Relaxed) as f64;
        let misses = self.l1_misses.load(std::sync::atomic::Ordering::Relaxed) as f64;
        let total = hits + misses;
        if total > 0.0 { hits / total } else { 0.0 }
    }

    pub fn get_avg_cache_time_us(&self) -> f64 {
        let total_time = self
            .total_cache_time_us
            .load(std::sync::atomic::Ordering::Relaxed) as f64;
        let total_queries = self
            .total_queries
            .load(std::sync::atomic::Ordering::Relaxed) as f64;
        if total_queries > 0.0 {
            total_time / total_queries
        } else {
            0.0
        }
    }
}

/// Cache key derived from search query
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CacheKey {
    query_hash: u64,
    mode: String,
    repositories_hash: u64,
    file_patterns_hash: u64,
    limit: usize,
    offset: usize,
}

impl CacheKey {
    fn from_query(query: &SearchQuery) -> Self {
        use std::collections::hash_map::DefaultHasher;

        let mut hasher = DefaultHasher::new();
        query.query.hash(&mut hasher);
        let query_hash = hasher.finish();

        let mut hasher = DefaultHasher::new();
        if let Some(repos) = &query.repositories {
            for repo in repos {
                repo.hash(&mut hasher);
            }
        }
        let repositories_hash = hasher.finish();

        let mut hasher = DefaultHasher::new();
        if let Some(patterns) = &query.file_patterns {
            for pattern in patterns {
                pattern.hash(&mut hasher);
            }
        }
        let file_patterns_hash = hasher.finish();

        Self {
            query_hash,
            mode: format!("{:?}", query.mode),
            repositories_hash,
            file_patterns_hash,
            limit: query.limit,
            offset: query.offset,
        }
    }
}

/// Cached search result with metadata
struct CachedResult {
    response: SearchResponse,
    cached_at: Instant,
    access_count: u32,
    last_accessed: Instant,
}

impl CachedResult {
    fn new(response: SearchResponse) -> Self {
        let now = Instant::now();
        Self {
            response,
            cached_at: now,
            access_count: 1,
            last_accessed: now,
        }
    }

    fn touch(&mut self) {
        self.access_count += 1;
        self.last_accessed = Instant::now();
    }

    fn is_expired(&self, ttl: Duration) -> bool {
        self.cached_at.elapsed() > ttl
    }
}

/// Multi-tier caching system for search results
pub struct MultiTierCache {
    /// L1: In-memory cache using DashMap for concurrent access
    l1_cache: Arc<DashMap<CacheKey, CachedResult>>,

    /// Cache configuration
    config: CacheConfig,

    /// Metrics for monitoring
    metrics: Arc<CacheMetrics>,
}

/// Configuration for the caching system
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Maximum number of entries in L1 cache
    pub l1_max_entries: usize,

    /// TTL for L1 cache entries
    pub l1_ttl: Duration,

    /// Minimum query length to cache (avoid caching single character queries)
    pub min_query_length: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            l1_max_entries: 10000,
            l1_ttl: Duration::from_secs(300), // 5 minutes
            min_query_length: 2,
        }
    }
}

impl MultiTierCache {
    pub fn new(
        config: CacheConfig,
        _storage: Option<Arc<crate::storage::StorageBackend>>, // Reserved for future L2 implementation
    ) -> Self {
        let cache = Self {
            l1_cache: Arc::new(DashMap::with_capacity(config.l1_max_entries)),
            config,
            metrics: Arc::new(CacheMetrics::default()),
        };

        // Start background cleanup task for expired entries
        cache.start_cleanup_task();

        cache
    }

    /// Get cached result if available
    pub async fn get(&self, query: &SearchQuery) -> Option<SearchResponse> {
        // Skip caching for very short queries
        if query.query.len() < self.config.min_query_length {
            return None;
        }

        let key = CacheKey::from_query(query);
        let start = Instant::now();

        self.metrics.record_query();

        // Check L1 cache
        if let Some(mut entry) = self.l1_cache.get_mut(&key) {
            if !entry.is_expired(self.config.l1_ttl) {
                entry.touch();
                self.metrics.record_hit();
                self.metrics
                    .record_cache_time(start.elapsed().as_micros() as u64);
                debug!("L1 cache hit for query: {}", query.query);
                return Some(entry.response.clone());
            } else {
                // Remove expired entry
                drop(entry);
                self.l1_cache.remove(&key);
                trace!("Removed expired L1 entry for query: {}", query.query);
            }
        }

        self.metrics.record_miss();
        self.metrics
            .record_cache_time(start.elapsed().as_micros() as u64);
        None
    }

    /// Store search result in cache
    pub async fn put(&self, query: &SearchQuery, response: SearchResponse) -> Result<()> {
        // Skip caching for very short queries
        if query.query.len() < self.config.min_query_length {
            return Ok(());
        }

        let key = CacheKey::from_query(query);

        // Evict LRU entry if at capacity
        if self.l1_cache.len() >= self.config.l1_max_entries {
            self.evict_lru();
        }

        // Store in L1
        let result = CachedResult::new(response);
        self.l1_cache.insert(key, result);
        debug!("Cached search result in L1 for query: {}", query.query);

        Ok(())
    }

    /// Invalidate cache entries matching a pattern
    pub async fn invalidate_pattern(&self, pattern: &str) {
        let mut removed_count = 0;

        self.l1_cache.retain(|key, _| {
            let should_keep = !key.query_hash.to_string().contains(pattern);
            if !should_keep {
                removed_count += 1;
            }
            should_keep
        });

        if removed_count > 0 {
            debug!(
                "Invalidated {} L1 cache entries matching pattern: {}",
                removed_count, pattern
            );
        }
    }

    /// Clear all cache entries
    pub async fn clear(&self) {
        let l1_size = self.l1_cache.len();
        self.l1_cache.clear();
        debug!("Cleared {} entries from L1 cache", l1_size);
    }

    /// Get cache metrics
    pub fn metrics(&self) -> Arc<CacheMetrics> {
        Arc::clone(&self.metrics)
    }

    /// Evict least recently used entry from L1
    fn evict_lru(&self) {
        let mut oldest_key = None;
        let mut oldest_time = Instant::now();

        // Find LRU entry
        for entry in self.l1_cache.iter() {
            if entry.value().last_accessed < oldest_time {
                oldest_time = entry.value().last_accessed;
                oldest_key = Some(entry.key().clone());
            }
        }

        if let Some(key) = oldest_key {
            self.l1_cache.remove(&key);
            trace!("Evicted LRU entry from L1 cache");
        }
    }

    /// Start background task to clean up expired entries
    fn start_cleanup_task(&self) {
        let cache = Arc::clone(&self.l1_cache);
        let ttl = self.config.l1_ttl;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));

            loop {
                interval.tick().await;

                let mut expired_count = 0;
                cache.retain(|_, entry| {
                    let should_keep = !entry.is_expired(ttl);
                    if !should_keep {
                        expired_count += 1;
                    }
                    should_keep
                });

                if expired_count > 0 {
                    trace!("Cleaned up {} expired L1 cache entries", expired_count);
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::SearchMode;

    #[tokio::test]
    async fn test_cache_key_generation() {
        let query1 = SearchQuery {
            query: "test query".to_string(),
            mode: SearchMode::Symbol,
            limit: 10,
            offset: 0,
            ..Default::default()
        };

        let query2 = SearchQuery {
            query: "test query".to_string(),
            mode: SearchMode::Symbol,
            limit: 10,
            offset: 0,
            ..Default::default()
        };

        let query3 = SearchQuery {
            query: "different query".to_string(),
            mode: SearchMode::Symbol,
            limit: 10,
            offset: 0,
            ..Default::default()
        };

        let key1 = CacheKey::from_query(&query1);
        let key2 = CacheKey::from_query(&query2);
        let key3 = CacheKey::from_query(&query3);

        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
    }

    #[tokio::test]
    async fn test_cache_expiration() {
        let config = CacheConfig {
            l1_ttl: Duration::from_millis(100),
            ..Default::default()
        };

        let cache = MultiTierCache::new(config, None);

        let query = SearchQuery {
            query: "test".to_string(),
            mode: SearchMode::Symbol,
            ..Default::default()
        };

        let response = SearchResponse {
            query: query.clone(),
            results: vec![],
            total_matches: 0,
            search_time_ms: 0,
            from_cache: None,
        };

        cache.put(&query, response.clone()).await.unwrap();

        // Should be in cache immediately
        assert!(cache.get(&query).await.is_some());

        // Wait for expiration
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Should be expired
        assert!(cache.get(&query).await.is_none());
    }

    #[tokio::test]
    async fn test_cache_metrics() {
        let cache = MultiTierCache::new(CacheConfig::default(), None);

        let query = SearchQuery {
            query: "test".to_string(),
            mode: SearchMode::Symbol,
            ..Default::default()
        };

        let response = SearchResponse {
            query: query.clone(),
            results: vec![],
            total_matches: 0,
            search_time_ms: 0,
            from_cache: None,
        };

        // Initial miss
        assert!(cache.get(&query).await.is_none());
        assert_eq!(
            cache
                .metrics
                .l1_misses
                .load(std::sync::atomic::Ordering::Relaxed),
            1
        );

        // Store in cache
        cache.put(&query, response).await.unwrap();

        // Cache hit
        assert!(cache.get(&query).await.is_some());
        assert_eq!(
            cache
                .metrics
                .l1_hits
                .load(std::sync::atomic::Ordering::Relaxed),
            1
        );

        // Hit rate should be 50% (1 hit, 1 miss)
        assert_eq!(cache.metrics.get_hit_rate(), 0.5);
    }
}
