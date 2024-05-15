use alloy_primitives::Address;
use brontes_metrics::db_cache::CacheData;
use brontes_types::db::{
    address_metadata::AddressMetadata, address_to_protocol_info::ProtocolInfo,
    searcher::SearcherInfo, token_info::TokenInfo,
};
use moka::{policy::EvictionPolicy, sync::SegmentedCache};

const MEGABYTE: usize = 1024 * 1024;

pub struct ReadWriteCache {
    address_meta:      SegmentedCache<Address, Option<AddressMetadata>, ahash::RandomState>,
    searcher_eoa:      SegmentedCache<Address, Option<SearcherInfo>, ahash::RandomState>,
    searcher_contract: SegmentedCache<Address, Option<SearcherInfo>, ahash::RandomState>,
    protocol_info:     SegmentedCache<Address, Option<ProtocolInfo>, ahash::RandomState>,
    token_info:        SegmentedCache<Address, Option<TokenInfo>, ahash::RandomState>,

    pub metrics: CacheData,
}

impl ReadWriteCache {
    pub fn new(memory_per_table_mb: usize) -> Self {
        let metrics = CacheData::default();
        Self {
            metrics,
            address_meta: SegmentedCache::builder(5)
                .eviction_policy(EvictionPolicy::lru())
                .max_capacity(
                    ((memory_per_table_mb * MEGABYTE) / std::mem::size_of::<AddressMetadata>())
                        as u64,
                )
                .build_with_hasher(ahash::RandomState::new()),

            searcher_eoa: SegmentedCache::builder(5)
                .eviction_policy(EvictionPolicy::lru())
                .max_capacity(
                    ((memory_per_table_mb * MEGABYTE) / std::mem::size_of::<SearcherInfo>()) as u64,
                )
                .build_with_hasher(ahash::RandomState::new()),

            searcher_contract: SegmentedCache::builder(5)
                .eviction_policy(EvictionPolicy::lru())
                .max_capacity(
                    ((memory_per_table_mb * MEGABYTE) / std::mem::size_of::<SearcherInfo>()) as u64,
                )
                .build_with_hasher(ahash::RandomState::new()),
            protocol_info: SegmentedCache::builder(5)
                .eviction_policy(EvictionPolicy::lru())
                .max_capacity(
                    ((memory_per_table_mb * MEGABYTE) / std::mem::size_of::<ProtocolInfo>()) as u64,
                )
                .build_with_hasher(ahash::RandomState::new()),

            token_info: SegmentedCache::builder(5)
                .eviction_policy(EvictionPolicy::lru())
                .max_capacity(
                    ((memory_per_table_mb * MEGABYTE) / std::mem::size_of::<TokenInfo>()) as u64,
                )
                .build_with_hasher(ahash::RandomState::new()),
        }
    }

    pub fn address_meta<R>(
        &self,
        read: bool,
        f: impl FnOnce(&SegmentedCache<Address, Option<AddressMetadata>, ahash::RandomState>) -> R,
    ) -> R {
        if read {
            self.metrics
                .clone()
                .cache_read::<R, AddressMetadata>("address_meta", || f(&self.address_meta))
        } else {
            self.metrics
                .clone()
                .cache_write::<R, AddressMetadata>("address_meta", || f(&self.address_meta))
        }
    }

    pub fn searcher_contract<R>(
        &self,
        read: bool,
        f: impl FnOnce(&SegmentedCache<Address, Option<SearcherInfo>, ahash::RandomState>) -> R,
    ) -> R {
        if read {
            self.metrics
                .clone()
                .cache_read::<R, SearcherInfo>("searcher_contract", || f(&self.searcher_contract))
        } else {
            self.metrics
                .clone()
                .cache_write::<R, SearcherInfo>("searcher_contract", || f(&self.searcher_contract))
        }
    }

    pub fn searcher_eoa<R>(
        &self,
        read: bool,
        f: impl FnOnce(&SegmentedCache<Address, Option<SearcherInfo>, ahash::RandomState>) -> R,
    ) -> R {
        if read {
            self.metrics
                .clone()
                .cache_read::<R, SearcherInfo>("searcher_eoa", || f(&self.searcher_eoa))
        } else {
            self.metrics
                .clone()
                .cache_write::<R, SearcherInfo>("searcher_eoa", || f(&self.searcher_eoa))
        }
    }

    pub fn protocol_info<R>(
        &self,
        read: bool,
        f: impl FnOnce(&SegmentedCache<Address, Option<ProtocolInfo>, ahash::RandomState>) -> R,
    ) -> R {
        if read {
            self.metrics
                .clone()
                .cache_read::<R, ProtocolInfo>("protocol_info", || f(&self.protocol_info))
        } else {
            self.metrics
                .clone()
                .cache_write::<R, ProtocolInfo>("protocol_info", || f(&self.protocol_info))
        }
    }

    pub fn token_info<R>(
        &self,
        read: bool,
        f: impl FnOnce(&SegmentedCache<Address, Option<TokenInfo>, ahash::RandomState>) -> R,
    ) -> R {
        if read {
            self.metrics
                .clone()
                .cache_read::<R, TokenInfo>("token_info", || f(&self.token_info))
        } else {
            self.metrics
                .clone()
                .cache_write::<R, TokenInfo>("token_info", || f(&self.token_info))
        }
    }
}
