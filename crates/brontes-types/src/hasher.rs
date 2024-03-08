//! default hashing types with custom hasher

use std::collections::{FastHashMap, FastHashSet};

use fasthash::RandomState;

#[cfg(target_pointer_width = "64")]
pub type FastHasher = RandomState<fasthash::xxh3::Hash64>;
#[cfg(target_pointer_width = "32")]
pub type FastHasher = RandomState<fasthash::xx::Hash32>;

/// FastHashMap using xx hash
pub type FastFastHashMap<K, V> = FastHashMap<K, V, FastHasher>;
/// FastHashSet using xx hash
pub type FastFastHashSet<V> = FastHashSet<V, FastHasher>;
