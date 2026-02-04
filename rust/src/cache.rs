// In-memory cache for database operations
//
// Provides fast lookups for frequently accessed data like fingerprints
// and test mappings using LruCache for automatic eviction.

use lru::LruCache;
use parking_lot::RwLock;
use std::num::NonZeroUsize;
use std::path::PathBuf;

use crate::types::Fingerprint;

/// LRU cache for database queries
pub struct Cache {
    /// Cached file fingerprints: path -> fingerprint
    fingerprints: RwLock<LruCache<PathBuf, Fingerprint>>,

    /// Cached test mappings: test_name -> list of checksums
    test_mappings: RwLock<LruCache<String, Vec<i32>>>,
}

impl Cache {
    /// Create a new cache with default size limit
    pub fn new() -> Self {
        Self::with_capacity(10_000)
    }

    /// Create a new cache with specified capacity
    pub fn with_capacity(max_size: usize) -> Self {
        let cap = NonZeroUsize::new(max_size).unwrap_or(NonZeroUsize::new(1).unwrap());
        Self {
            fingerprints: RwLock::new(LruCache::new(cap)),
            test_mappings: RwLock::new(LruCache::new(cap)),
        }
    }

    /// Get a fingerprint from cache (promotes to most-recently-used)
    pub fn get_fingerprint(&self, path: &PathBuf) -> Option<Fingerprint> {
        self.fingerprints.write().get(path).cloned()
    }

    /// Insert a fingerprint into cache (auto-evicts LRU entry when full)
    pub fn insert_fingerprint(&self, path: PathBuf, fp: Fingerprint) {
        self.fingerprints.write().put(path, fp);
    }

    /// Get test mapping from cache (promotes to most-recently-used)
    #[allow(dead_code)]
    pub fn get_test_mapping(&self, test_name: &str) -> Option<Vec<i32>> {
        self.test_mappings.write().get(test_name).cloned()
    }

    /// Insert test mapping into cache (auto-evicts LRU entry when full)
    #[allow(dead_code)]
    pub fn insert_test_mapping(&self, test_name: String, checksums: Vec<i32>) {
        self.test_mappings.write().put(test_name, checksums);
    }

    /// Clear all cached data
    pub fn clear(&self) {
        self.fingerprints.write().clear();
        self.test_mappings.write().clear();
    }
}

impl Default for Cache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Fingerprint;
    use std::path::PathBuf;

    #[test]
    fn test_cache_fingerprint() {
        let cache = Cache::new();
        let path = PathBuf::from("test.py");

        let fp = Fingerprint {
            filename: "test.py".to_string(),
            checksums: vec![123, 456],
            file_hash: "abc".to_string(),
            mtime: 1.0,
            blocks: None,
        };

        cache.insert_fingerprint(path.clone(), fp.clone());
        let retrieved = cache.get_fingerprint(&path).unwrap();

        assert_eq!(retrieved.filename, fp.filename);
        assert_eq!(retrieved.checksums, fp.checksums);
    }

    #[test]
    fn test_cache_test_mapping() {
        let cache = Cache::new();

        cache.insert_test_mapping("test_foo".to_string(), vec![1, 2, 3]);
        let retrieved = cache.get_test_mapping("test_foo").unwrap();

        assert_eq!(retrieved, vec![1, 2, 3]);
    }

    #[test]
    fn test_cache_clear() {
        let cache = Cache::new();
        let path = PathBuf::from("test.py");

        let fp = Fingerprint {
            filename: "test.py".to_string(),
            checksums: vec![123],
            file_hash: "abc".to_string(),
            mtime: 1.0,
            blocks: None,
        };

        cache.insert_fingerprint(path.clone(), fp);
        cache.clear();

        assert!(cache.get_fingerprint(&path).is_none());
    }

    #[test]
    fn test_lru_eviction_order() {
        // Create a cache with capacity 2
        let cache = Cache::with_capacity(2);

        let fp1 = Fingerprint {
            filename: "a.py".to_string(),
            checksums: vec![1],
            file_hash: "h1".to_string(),
            mtime: 1.0,
            blocks: None,
        };
        let fp2 = Fingerprint {
            filename: "b.py".to_string(),
            checksums: vec![2],
            file_hash: "h2".to_string(),
            mtime: 2.0,
            blocks: None,
        };
        let fp3 = Fingerprint {
            filename: "c.py".to_string(),
            checksums: vec![3],
            file_hash: "h3".to_string(),
            mtime: 3.0,
            blocks: None,
        };

        cache.insert_fingerprint(PathBuf::from("a.py"), fp1);
        cache.insert_fingerprint(PathBuf::from("b.py"), fp2);

        // Access a.py to make it most-recently-used
        assert!(cache.get_fingerprint(&PathBuf::from("a.py")).is_some());

        // Insert c.py — should evict b.py (least recently used)
        cache.insert_fingerprint(PathBuf::from("c.py"), fp3);

        assert!(cache.get_fingerprint(&PathBuf::from("a.py")).is_some());
        assert!(cache.get_fingerprint(&PathBuf::from("b.py")).is_none()); // evicted
        assert!(cache.get_fingerprint(&PathBuf::from("c.py")).is_some());
    }
}
