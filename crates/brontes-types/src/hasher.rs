//! default hashing types with custom hasher
use std::collections::{HashMap, HashSet};

use ahash::RandomState;

pub type FastHasher = RandomState;
/// FastHashMap using ahash
pub type FastHashMap<K, V> = HashMap<K, V, FastHasher>;
/// FastHashSet using ahash
pub type FastHashSet<V> = HashSet<V, FastHasher>;
