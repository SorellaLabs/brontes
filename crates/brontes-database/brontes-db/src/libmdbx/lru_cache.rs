use std::task::Poll;

use brontes_types::{
    db::{address_metadata::AddressMetadata, searcher::SearcherInfo},
    UnboundedYapperReceiver,
};
use futures::Future;
use reth_primitives::Address;
use schnellru::{ByMemoryUsage, LruMap};

pub enum CacheMsg {
    Fetch(TryCacheFetch),
    Update(bool, CacheUpdate),
}

const MEGABYTE: usize = 1024 * 1024;

pub enum TryCacheFetch {
    AddressMeta(Address, tokio::sync::oneshot::Sender<Option<AddressMetadata>>),
    SearcherEoa(Address, tokio::sync::oneshot::Sender<Option<SearcherInfo>>),
    SearcherContract(Address, tokio::sync::oneshot::Sender<Option<SearcherInfo>>),
}

/// bool at end means it was from a write and to update.
pub enum CacheUpdate {
    AddressMeta(Address, AddressMetadata),
    SearcherEoa(Address, SearcherInfo),
    SearcherContract(Address, SearcherInfo),
}

/// reduces the amount of small table queries needed
pub struct LibmdbxLRUCache {
    rx: UnboundedYapperReceiver<CacheMsg>,

    address_meta:      LruMap<Address, AddressMetadata, ByMemoryUsage, ahash::RandomState>,
    searcher_eoa:      LruMap<Address, SearcherInfo, ByMemoryUsage, ahash::RandomState>,
    searcher_contract: LruMap<Address, SearcherInfo, ByMemoryUsage, ahash::RandomState>,
}
impl LibmdbxLRUCache {
    pub fn new(rx: UnboundedYapperReceiver<CacheMsg>, memory_per_table_mb: usize) -> Self {
        Self {
            rx,
            address_meta: LruMap::with_hasher(
                ByMemoryUsage::new(memory_per_table_mb * MEGABYTE),
                ahash::RandomState::new(),
            ),
            searcher_eoa: LruMap::with_hasher(
                ByMemoryUsage::new(memory_per_table_mb * MEGABYTE),
                ahash::RandomState::new(),
            ),
            searcher_contract: LruMap::with_hasher(
                ByMemoryUsage::new(memory_per_table_mb * MEGABYTE),
                ahash::RandomState::new(),
            ),
        }
    }
}

impl Future for LibmdbxLRUCache {
    type Output = ();

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let mut work = 2048;
        let this = self.get_mut();
        loop {
            // work -= 1;
            // if work == 0 {
            //     cx.waker().wake_by_ref();
            //     return Poll::Pending
            // }

            if let Poll::Ready(v) = this.rx.poll_recv(cx) {
                match v {
                    Some(v) => match v {
                        CacheMsg::Fetch(f) => match f {
                            TryCacheFetch::AddressMeta(addr, tx) => {
                                let _ = tx.send(this.address_meta.get(&addr).cloned());
                            }
                            TryCacheFetch::SearcherEoa(addr, tx) => {
                                let _ = tx.send(this.searcher_eoa.get(&addr).cloned());
                            }
                            TryCacheFetch::SearcherContract(addr, tx) => {
                                let _ = tx.send(this.searcher_contract.get(&addr).cloned());
                            }
                        },
                        CacheMsg::Update(write, update) => match update {
                            CacheUpdate::AddressMeta(key, value) => {
                                if write {
                                    // always overwrite
                                    this.address_meta.insert(key, value);
                                } else {
                                    // only overwrite if non-existent
                                    this.address_meta.get_or_insert(key, || value);
                                }
                            }
                            CacheUpdate::SearcherEoa(key, value) => {
                                if write {
                                    // always overwrite
                                    this.searcher_eoa.insert(key, value);
                                } else {
                                    // only overwrite if non-existent
                                    this.searcher_eoa.get_or_insert(key, || value);
                                }
                            }
                            CacheUpdate::SearcherContract(key, value) => {
                                if write {
                                    // always overwrite
                                    this.searcher_contract.insert(key, value);
                                } else {
                                    // only overwrite if non-existent
                                    this.searcher_contract.get_or_insert(key, || value);
                                }
                            }
                        },
                    },
                    None => return Poll::Ready(()),
                }
            } else {
                tracing::error!("somehow empty");
            }
        }
    }
}
