//! default hashing types with custom hasher
use std::collections::{HashMap, HashSet};

use ahash::RandomState;

pub type FastHasher = RandomState;
/// FastHashMap using xx hash
pub type FastHashMap<K, V> = HashMap<K, V, FastHasher>;
/// FastHashSet using xx hash
pub type FastHashSet<V> = HashSet<V, FastHasher>;
