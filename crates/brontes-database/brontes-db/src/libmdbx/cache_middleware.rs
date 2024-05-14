use alloy_primitives::Address;
use brontes_metrics::db_cache::CacheData;
use brontes_types::db::{
    address_metadata::AddressMetadata, address_to_protocol_info::ProtocolInfo,
    searcher::SearcherInfo, token_info::TokenInfo,
};
use moka::{policy::EvictionPolicy, sync::SegmentedCache};

const MEGABYTE: usize = 1024 * 1024;

/// allows for us to avoid lock conflicts
pub struct ReadWriteMultiplex<const N: usize> {
    cache: [ReadWriteCache; N],
}

impl<const N: usize> ReadWriteMultiplex<N> {
    pub fn new(memory_per_table_mb: usize) -> Self {
        println!("opening cache");
        let memory_per_shard = memory_per_table_mb / N;
        let metrics = CacheData::default();
        let cache =
            core::array::from_fn(|_| ReadWriteCache::new(memory_per_shard, metrics.clone()));

        println!("finished  cache");

        Self { cache }
    }

    pub fn cache<R>(&self, key: Address, f: impl FnOnce(&ReadWriteCache) -> R) -> R {
        // let mut hasher = ahash::AHasher::default();
        // key.hash(&mut hasher);
        // let key = hasher.finish() as usize;
        // let shard = key % N;

        f(&self.cache[0])
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
            // let mut hasher = ahash::AHasher::default();
            // addr.hash(&mut hasher);
            // let key = hasher.finish() as usize;
            // let shard = key % N;
            shards[0].push(addr);
        }

        for (key, shard_data) in shards.into_iter().enumerate() {
            out.extend(f(shard_data, &self.cache[key]));
        }

        collect(out)
    }

    pub fn write(&self, key: Address, f: impl FnOnce(&ReadWriteCache)) {
        // let mut hasher = ahash::AHasher::default();
        // key.hash(&mut hasher);
        // let key = hasher.finish() as usize;
        // let shard = key % N;
        f(&self.cache[0])
    }
}

pub struct ReadWriteCache {
    pub address_meta:      SegmentedCache<Address, Option<AddressMetadata>, ahash::RandomState>,
    pub searcher_eoa:      SegmentedCache<Address, Option<SearcherInfo>, ahash::RandomState>,
    pub searcher_contract: SegmentedCache<Address, Option<SearcherInfo>, ahash::RandomState>,
    pub protocol_info:     SegmentedCache<Address, Option<ProtocolInfo>, ahash::RandomState>,
    pub token_info:        SegmentedCache<Address, Option<TokenInfo>, ahash::RandomState>,

    pub metrics: CacheData,
}

impl ReadWriteCache {
    pub fn new(memory_per_table_mb: usize, metrics: CacheData) -> Self {
        println!("starting cache");
        Self {
            metrics,
            address_meta: SegmentedCache::builder(100)
                .eviction_policy(EvictionPolicy::lru())
                .max_capacity(
                    ((memory_per_table_mb * MEGABYTE) / std::mem::size_of::<AddressMetadata>())
                        as u64,
                )
                .build_with_hasher(ahash::RandomState::new()),

            searcher_eoa: SegmentedCache::builder(100)
                .eviction_policy(EvictionPolicy::lru())
                .max_capacity(
                    ((memory_per_table_mb * MEGABYTE) / std::mem::size_of::<SearcherInfo>()) as u64,
                )
                .build_with_hasher(ahash::RandomState::new()),

            searcher_contract: SegmentedCache::builder(100)
                .eviction_policy(EvictionPolicy::lru())
                .max_capacity(
                    ((memory_per_table_mb * MEGABYTE) / std::mem::size_of::<SearcherInfo>()) as u64,
                )
                .build_with_hasher(ahash::RandomState::new()),
            protocol_info: SegmentedCache::builder(100)
                .eviction_policy(EvictionPolicy::lru())
                .max_capacity(
                    ((memory_per_table_mb * MEGABYTE) / std::mem::size_of::<ProtocolInfo>()) as u64,
                )
                .build_with_hasher(ahash::RandomState::new()),

            token_info: SegmentedCache::builder(100)
                .eviction_policy(EvictionPolicy::lru())
                .max_capacity(
                    ((memory_per_table_mb * MEGABYTE) / std::mem::size_of::<TokenInfo>()) as u64,
                )
                .build_with_hasher(ahash::RandomState::new()),
        }
    }
}
