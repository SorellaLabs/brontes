use alloy_primitives::Address;
use futures::Future;

use crate::{
    db::{dex::DexQuotes, searcher::SearcherInfo},
    mev::{Bundle, MevBlock},
    pair::Pair,
    structured_trace::TxTrace,
    Protocol, SubGraphEdge,
};

#[auto_impl::auto_impl(&)]
pub trait DBWriter: Send + Sync + Unpin + 'static {
    /// allows for writing results to multiple databases
    type Inner: DBWriter;

    fn inner(&self) -> &Self::Inner;

    fn write_dex_quotes(
        &self,
        block_number: u64,
        quotes: Option<DexQuotes>,
    ) -> impl Future<Output = eyre::Result<()>> + Send + Sync {
        self.inner().write_dex_quotes(block_number, quotes)
    }

    fn write_token_info(
        &self,
        address: Address,
        decimals: u8,
        symbol: String,
    ) -> impl Future<Output = eyre::Result<()>> + Send + Sync {
        self.inner().write_token_info(address, decimals, symbol)
    }

    fn save_pair_at(
        &self,
        block: u64,
        pair: Pair,
        edges: Vec<SubGraphEdge>,
    ) -> impl Future<Output = eyre::Result<()>> + Send + Sync {
        self.inner().save_pair_at(block, pair, edges)
    }

    fn save_mev_blocks(
        &self,
        block_number: u64,
        block: MevBlock,
        mev: Vec<Bundle>,
    ) -> impl Future<Output = eyre::Result<()>> + Send + Sync {
        self.inner().save_mev_blocks(block_number, block, mev)
    }

    fn write_searcher_info(
        &self,
        searcher_eoa: Address,
        searcher_info: SearcherInfo,
    ) -> impl Future<Output = eyre::Result<()>> + Send + Sync {
        self.inner()
            .write_searcher_info(searcher_eoa, searcher_info)
    }

    fn insert_pool(
        &self,
        block: u64,
        address: Address,
        tokens: [Address; 2],
        classifier_name: Protocol,
    ) -> impl Future<Output = eyre::Result<()>> + Send + Sync {
        self.inner()
            .insert_pool(block, address, tokens, classifier_name)
    }

    fn save_traces(
        &self,
        block: u64,
        traces: Vec<TxTrace>,
    ) -> impl Future<Output = eyre::Result<()>> + Send + Sync {
        self.inner().save_traces(block, traces)
    }
}
