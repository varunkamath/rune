// LRU Cache implementation with TTL support

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Entry in the cache with value and metadata
#[derive(Clone)]
struct CacheEntry<V> {
    value: V,
    last_accessed: Instant,
    expiry: Option<Instant>,
}

/// Thread-safe LRU cache with time-to-live support
pub struct LRUCache<K, V>
where
    K: Eq + std::hash::Hash + Clone,
    V: Clone,
{
    capacity: usize,
    store: Arc<Mutex<HashMap<K, CacheEntry<V>>>>,
    access_order: Arc<Mutex<VecDeque<K>>>,
    default_ttl: Option<Duration>,
}

impl<K, V> LRUCache<K, V>
where
    K: Eq + std::hash::Hash + Clone,
    V: Clone,
{
    /// Create new LRU cache with specified capacity
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            store: Arc::new(Mutex::new(HashMap::with_capacity(capacity))),
            access_order: Arc::new(Mutex::new(VecDeque::with_capacity(capacity))),
            default_ttl: None,
        }
    }

    /// Create cache with default TTL for entries
    pub fn with_ttl(capacity: usize, ttl: Duration) -> Self {
        Self {
            capacity,
            store: Arc::new(Mutex::new(HashMap::with_capacity(capacity))),
            access_order: Arc::new(Mutex::new(VecDeque::with_capacity(capacity))),
            default_ttl: Some(ttl),
        }
    }

    /// Insert or update value in cache
    pub fn put(&self, key: K, value: V) -> Option<V> {
        self.put_with_ttl(key, value, self.default_ttl)
    }

    /// Insert value with custom TTL
    pub fn put_with_ttl(&self, key: K, value: V, ttl: Option<Duration>) -> Option<V> {
        let mut store = self.store.lock().unwrap();
        let mut order = self.access_order.lock().unwrap();

        // Remove from access order if exists
        if let Some(pos) = order.iter().position(|k| k == &key) {
            order.remove(pos);
        }

        // Evict oldest if at capacity
        if store.len() >= self.capacity && !store.contains_key(&key) {
            if let Some(oldest_key) = order.pop_back() {
                store.remove(&oldest_key);
            }
        }

        // Insert new entry
        let expiry = ttl.map(|d| Instant::now() + d);
        let entry = CacheEntry {
            value: value.clone(),
            last_accessed: Instant::now(),
            expiry,
        };

        order.push_front(key.clone());
        store.insert(key, entry).map(|e| e.value)
    }

    /// Retrieve value from cache
    pub fn get(&self, key: &K) -> Option<V> {
        let mut store = self.store.lock().unwrap();
        let mut order = self.access_order.lock().unwrap();

        if let Some(entry) = store.get_mut(key) {
            // Check if expired
            if let Some(expiry) = entry.expiry {
                if Instant::now() > expiry {
                    store.remove(key);
                    if let Some(pos) = order.iter().position(|k| k == key) {
                        order.remove(pos);
                    }
                    return None;
                }
            }

            // Update access order
            if let Some(pos) = order.iter().position(|k| k == key) {
                order.remove(pos);
            }
            order.push_front(key.clone());

            // Update last accessed time
            entry.last_accessed = Instant::now();

            Some(entry.value.clone())
        } else {
            None
        }
    }

    /// Remove entry from cache
    pub fn remove(&self, key: &K) -> Option<V> {
        let mut store = self.store.lock().unwrap();
        let mut order = self.access_order.lock().unwrap();

        if let Some(pos) = order.iter().position(|k| k == key) {
            order.remove(pos);
        }

        store.remove(key).map(|e| e.value)
    }

    /// Clear all entries from cache
    pub fn clear(&self) {
        let mut store = self.store.lock().unwrap();
        let mut order = self.access_order.lock().unwrap();

        store.clear();
        order.clear();
    }

    /// Get current size of cache
    pub fn size(&self) -> usize {
        self.store.lock().unwrap().len()
    }

    /// Check if key exists and is not expired
    pub fn contains_key(&self, key: &K) -> bool {
        let store = self.store.lock().unwrap();

        if let Some(entry) = store.get(key) {
            if let Some(expiry) = entry.expiry {
                return Instant::now() <= expiry;
            }
            true
        } else {
            false
        }
    }

    /// Remove expired entries
    pub fn evict_expired(&self) -> usize {
        let mut store = self.store.lock().unwrap();
        let mut order = self.access_order.lock().unwrap();

        let now = Instant::now();
        let mut evicted = 0;

        let expired_keys: Vec<K> = store
            .iter()
            .filter_map(|(k, v)| {
                if let Some(expiry) = v.expiry {
                    if now > expiry {
                        return Some(k.clone());
                    }
                }
                None
            })
            .collect();

        for key in expired_keys {
            store.remove(&key);
            if let Some(pos) = order.iter().position(|k| k == &key) {
                order.remove(pos);
            }
            evicted += 1;
        }

        evicted
    }
}

/// Cache statistics for monitoring
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub expired: u64,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}
