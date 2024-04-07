//! default hashing types with custom hasher
use std::collections::{HashMap, HashSet};

use ahash::RandomState;

pub type FastHasher = RandomState;
/// FastHashMap using ahash
pub type FastHashMap<K, V> = HashMap<K, V, FastHasher>;
/// FastHashSet using ahash
pub type FastHashSet<V> = HashSet<V, FastHasher>;

/// Creates a new FastHashMap with ahash::RandomState hasher
pub fn new_fast_hash_map<K, V>() -> FastHashMap<K, V> {
    FastHashMap::with_hasher(FastHasher::new())
}

/// Creates a new FastHashSet with ahash::RandomState hasher
pub fn new_fast_hash_set<V>() -> FastHashSet<V> {
    FastHashSet::with_hasher(FastHasher::new())
}
