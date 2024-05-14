use std::hash::{Hash, Hasher};

use alloy_primitives::Address;
use brontes_types::{
    db::{
        address_metadata::AddressMetadata, address_to_protocol_info::ProtocolInfo,
        searcher::SearcherInfo, token_info::TokenInfo,
    },
    FastHashMap,
};
use schnellru::{ByMemoryUsage, LruMap};

const MEGABYTE: usize = 1024 * 1024;

/// allows for us to avoid lock conflicts
pub struct ReadWriteMultiplex<const N: usize> {
    cache: [ReadWriteCache; N],
}

impl<const N: usize> ReadWriteMultiplex<N> {
    pub fn new(memory_per_table_mb: usize) -> Self {
        let memory_per_shard = memory_per_table_mb / N;
        let cache = core::array::from_fn(|_| ReadWriteCache::new(memory_per_shard));

        Self { cache }
    }

    pub fn cache<R>(&self, key: Address, f: impl FnOnce(&ReadWriteCache) -> R) -> R {
        let mut hasher = ahash::AHasher::default();
        key.hash(&mut hasher);
        let key = hasher.finish() as usize;
        let shard = key % N;

        f(&self.cache[shard])
    }

    pub fn multi_cache<R, O>(
        &self,
        keys: Vec<Address>,
        f: impl Fn(Vec<Address>, &ReadWriteCache) -> Vec<(Address, R)>,
        collect: impl FnOnce(Vec<(Address, R)>) -> O,
    ) -> O {
        let mut out = Vec::with_capacity(keys.len());
        let mut shards: Vec<Vec<Address>> = vec![vec![]; N];

        for addr in keys {
            let mut hasher = ahash::AHasher::default();
            addr.hash(&mut hasher);
            let key = hasher.finish() as usize;
            let shard = key % N;
            shards[shard].push(addr);
        }

        for (key, shard_data) in shards.into_iter().enumerate() {
            out.extend(f(shard_data, &self.cache[key]));
        }

        collect(out)
    }

    pub fn write(&self, key: Address, f: impl FnOnce(&ReadWriteCache)) {
        let mut hasher = ahash::AHasher::default();
        key.hash(&mut hasher);
        let key = hasher.finish() as usize;
        let shard = key % N;
        f(&self.cache[shard])
    }
}

pub struct ReadWriteCache {
    pub address_meta: parking_lot::Mutex<
        LruMap<Address, Option<AddressMetadata>, ByMemoryUsage, ahash::RandomState>,
    >,
    pub searcher_eoa: parking_lot::Mutex<
        LruMap<Address, Option<SearcherInfo>, ByMemoryUsage, ahash::RandomState>,
    >,
    pub searcher_contract: parking_lot::Mutex<
        LruMap<Address, Option<SearcherInfo>, ByMemoryUsage, ahash::RandomState>,
    >,
    pub protocol_info: parking_lot::Mutex<
        LruMap<Address, Option<ProtocolInfo>, ByMemoryUsage, ahash::RandomState>,
    >,
    pub token_info:
        parking_lot::Mutex<LruMap<Address, Option<TokenInfo>, ByMemoryUsage, ahash::RandomState>>,
}

impl ReadWriteCache {
    pub fn new(memory_per_table_mb: usize) -> Self {
        Self {
            address_meta:      LruMap::with_hasher(
                ByMemoryUsage::new(memory_per_table_mb * MEGABYTE),
                ahash::RandomState::new(),
            )
            .into(),
            searcher_eoa:      LruMap::with_hasher(
                ByMemoryUsage::new(memory_per_table_mb * MEGABYTE),
                ahash::RandomState::new(),
            )
            .into(),
            searcher_contract: LruMap::with_hasher(
                ByMemoryUsage::new(memory_per_table_mb * MEGABYTE),
                ahash::RandomState::new(),
            )
            .into(),
            protocol_info:     LruMap::with_hasher(
                ByMemoryUsage::new(memory_per_table_mb * MEGABYTE),
                ahash::RandomState::new(),
            )
            .into(),
            token_info:        LruMap::with_hasher(
                ByMemoryUsage::new(memory_per_table_mb * MEGABYTE),
                ahash::RandomState::new(),
            )
            .into(),
        }
    }
}
