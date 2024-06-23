use std::sync::Arc;

use alloy_primitives::Address;
use brontes_metrics::db_cache::CacheData;
use brontes_types::db::{
    address_metadata::AddressMetadata, address_to_protocol_info::ProtocolInfo,
    searcher::SearcherInfo, token_info::TokenInfo,
};
use moka::{policy::EvictionPolicy, sync::SegmentedCache};

const MEGABYTE: usize = 1024 * 1024;

#[derive(Clone)]
pub struct ReadWriteCache {
    address_meta:      Arc<SegmentedCache<Address, Option<AddressMetadata>, ahash::RandomState>>,
    searcher_eoa:      Arc<SegmentedCache<Address, Option<SearcherInfo>, ahash::RandomState>>,
    searcher_contract: Arc<SegmentedCache<Address, Option<SearcherInfo>, ahash::RandomState>>,
    protocol_info:     Arc<SegmentedCache<Address, Option<ProtocolInfo>, ahash::RandomState>>,
    token_info:        Arc<SegmentedCache<Address, Option<TokenInfo>, ahash::RandomState>>,

    pub metrics: Option<CacheData>,
}

impl ReadWriteCache {
    pub fn new(memory_per_table_mb: usize, metrics: bool) -> Self {
        let metrics = metrics.then(|| CacheData::default());
        Self {
            metrics,
            address_meta: SegmentedCache::builder(200)
                .eviction_policy(EvictionPolicy::lru())
                .max_capacity(
                    ((memory_per_table_mb * MEGABYTE) / std::mem::size_of::<AddressMetadata>())
                        as u64,
                )
                .build_with_hasher(ahash::RandomState::new())
                .into(),

            searcher_eoa: SegmentedCache::builder(200)
                .eviction_policy(EvictionPolicy::lru())
                .max_capacity(
                    ((memory_per_table_mb * MEGABYTE) / std::mem::size_of::<SearcherInfo>()) as u64,
                )
                .build_with_hasher(ahash::RandomState::new())
                .into(),

            searcher_contract: SegmentedCache::builder(200)
                .eviction_policy(EvictionPolicy::lru())
                .max_capacity(
                    ((memory_per_table_mb * MEGABYTE) / std::mem::size_of::<SearcherInfo>()) as u64,
                )
                .build_with_hasher(ahash::RandomState::new())
                .into(),
            protocol_info: SegmentedCache::builder(200)
                .eviction_policy(EvictionPolicy::lru())
                .max_capacity(
                    ((memory_per_table_mb * MEGABYTE) / std::mem::size_of::<ProtocolInfo>()) as u64,
                )
                .build_with_hasher(ahash::RandomState::new())
                .into(),

            token_info: SegmentedCache::builder(200)
                .eviction_policy(EvictionPolicy::lru())
                .max_capacity(
                    ((memory_per_table_mb * MEGABYTE) / std::mem::size_of::<TokenInfo>()) as u64,
                )
                .build_with_hasher(ahash::RandomState::new())
                .into(),
        }
    }

    fn record_metrics<R, T, TY>(
        &self,
        read: bool,
        name: &str,
        cache: &T,
        f: impl FnOnce(&T) -> R,
    ) -> R {
        if let Some(metrics) = self.metrics.clone() {
            if read {
                metrics.cache_read::<R, TY>(name, || f(cache))
            } else {
                metrics.cache_write::<R, TY>(name, || f(cache))
            }
        } else {
            f(cache)
        }
    }

    pub fn address_meta<R>(
        &self,
        read: bool,
        f: impl FnOnce(&SegmentedCache<Address, Option<AddressMetadata>, ahash::RandomState>) -> R,
    ) -> R {
        self.record_metrics::<R, _, AddressMetadata>(read, "address_meta", &*self.address_meta, f)
    }

    pub fn searcher_contract<R>(
        &self,
        read: bool,
        f: impl FnOnce(&SegmentedCache<Address, Option<SearcherInfo>, ahash::RandomState>) -> R,
    ) -> R {
        self.record_metrics::<R, _, SearcherInfo>(
            read,
            "searcher_contract",
            &*self.searcher_contract,
            f,
        )
    }

    pub fn searcher_eoa<R>(
        &self,
        read: bool,
        f: impl FnOnce(&SegmentedCache<Address, Option<SearcherInfo>, ahash::RandomState>) -> R,
    ) -> R {
        self.record_metrics::<R, _, SearcherInfo>(read, "searcher_eoa", &*self.searcher_eoa, f)
    }

    pub fn protocol_info<R>(
        &self,
        read: bool,
        f: impl FnOnce(&SegmentedCache<Address, Option<ProtocolInfo>, ahash::RandomState>) -> R,
    ) -> R {
        self.record_metrics::<R, _, ProtocolInfo>(read, "protocol_info", &*self.protocol_info, f)
    }

    pub fn token_info<R>(
        &self,
        read: bool,
        f: impl FnOnce(&SegmentedCache<Address, Option<TokenInfo>, ahash::RandomState>) -> R,
    ) -> R {
        self.record_metrics::<R, _, TokenInfo>(read, "token_info", &*self.token_info, f)
    }
}
