//! default hashing types with custom hasher

use std::collections::{HashMap, HashSet};

use fasthash::RandomState;

#[cfg(target_pointer_width = "64")]
pub type FastHasher = RandomState<fasthash::xxh3::Hash64>;
#[cfg(target_pointer_width = "32")]
pub type FastHasher = RandomState<fasthash::xx::Hash32>;

/// FastHashMap using xx hash
pub type FastHashMap<K, V> = HashMap<K, V, FastHasher>;
/// FastHashSet using xx hash
pub type FastHashSet<V> = HashSet<V, FastHasher>;
