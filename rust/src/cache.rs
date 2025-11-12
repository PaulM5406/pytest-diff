// In-memory cache for database operations
//
// Provides fast lookups for frequently accessed data like fingerprints
// and test mappings using DashMap for concurrent access.

use dashmap::DashMap;
use std::path::PathBuf;

use crate::types::Fingerprint;

/// LRU-style cache for database queries
pub struct Cache {
    /// Cached file fingerprints: path -> fingerprint
    fingerprints: DashMap<PathBuf, Fingerprint>,

    /// Cached test mappings: test_name -> list of checksums
    test_mappings: DashMap<String, Vec<i32>>,

    /// Maximum number of fingerprints to cache
    max_size: usize,
}

impl Cache {
    /// Create a new cache with default size limit
    pub fn new() -> Self {
        Self::with_capacity(10_000)
    }

    /// Create a new cache with specified capacity
    pub fn with_capacity(max_size: usize) -> Self {
        Self {
            fingerprints: DashMap::new(),
            test_mappings: DashMap::new(),
            max_size,
        }
    }

    /// Get a fingerprint from cache
    pub fn get_fingerprint(&self, path: &PathBuf) -> Option<Fingerprint> {
        self.fingerprints.get(path).map(|v| v.clone())
    }

    /// Insert a fingerprint into cache
    pub fn insert_fingerprint(&self, path: PathBuf, fp: Fingerprint) {
        if self.fingerprints.len() >= self.max_size {
            self.evict_fingerprints();
        }
        self.fingerprints.insert(path, fp);
    }

    /// Get test mapping from cache
    #[allow(dead_code)]
    pub fn get_test_mapping(&self, test_name: &str) -> Option<Vec<i32>> {
        self.test_mappings.get(test_name).map(|v| v.clone())
    }

    /// Insert test mapping into cache
    #[allow(dead_code)]
    pub fn insert_test_mapping(&self, test_name: String, checksums: Vec<i32>) {
        self.test_mappings.insert(test_name, checksums);
    }

    /// Evict oldest entries when cache is full
    fn evict_fingerprints(&self) {
        // Simple eviction: clear 10% of entries
        let to_remove = self.max_size / 10;
        let mut removed = 0;

        self.fingerprints.retain(|_, _| {
            if removed < to_remove {
                removed += 1;
                false // Remove this entry
            } else {
                true // Keep this entry
            }
        });
    }

    /// Clear all cached data
    pub fn clear(&self) {
        self.fingerprints.clear();
        self.test_mappings.clear();
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
}
