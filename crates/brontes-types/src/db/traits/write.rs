use alloy_primitives::Address;
use futures::Future;

use crate::{
    db::{
        builder::{BuilderInfo, BuilderStats},
        dex::DexQuotes,
        searcher::{SearcherInfo, SearcherStats},
    },
    mev::{Bundle, MevBlock},
    pair::Pair,
    structured_trace::TxTrace,
    Protocol, SubGraphEdge,
};

#[auto_impl::auto_impl(&)]
pub trait DBWriter: Send + Unpin + 'static {
    /// allows for writing results to multiple databases
    type Inner: DBWriter;

    fn inner(&self) -> &Self::Inner;

    fn write_dex_quotes(
        &self,
        block_number: u64,
        quotes: Option<DexQuotes>,
    ) -> impl Future<Output = eyre::Result<()>> + Send {
        self.inner().write_dex_quotes(block_number, quotes)
    }

    fn write_token_info(
        &self,
        address: Address,
        decimals: u8,
        symbol: String,
    ) -> impl Future<Output = eyre::Result<()>> + Send {
        self.inner().write_token_info(address, decimals, symbol)
    }

    fn save_pair_at(&self, block: u64, pair: Pair, edges: Vec<SubGraphEdge>) -> eyre::Result<()> {
        self.inner().save_pair_at(block, pair, edges)
    }

    fn save_mev_blocks(
        &self,
        block_number: u64,
        block: MevBlock,
        mev: Vec<Bundle>,
    ) -> impl Future<Output = eyre::Result<()>> + Send {
        self.inner().save_mev_blocks(block_number, block, mev)
    }

    fn write_searcher_info(
        &self,
        eoa_address: Address,
        contract_address: Option<Address>,
        eoa_info: SearcherInfo,
        contract_info: Option<SearcherInfo>,
    ) -> impl Future<Output = eyre::Result<()>> + Send {
        self.inner()
            .write_searcher_info(eoa_address, contract_address, eoa_info, contract_info)
    }

    fn write_searcher_eoa_info(
        &self,
        searcher_eoa: Address,
        searcher_info: SearcherInfo,
    ) -> impl Future<Output = eyre::Result<()>> + Send {
        self.inner()
            .write_searcher_eoa_info(searcher_eoa, searcher_info)
    }

    fn write_searcher_contract_info(
        &self,
        searcher_contract: Address,
        searcher_info: SearcherInfo,
    ) -> impl Future<Output = eyre::Result<()>> + Send {
        self.inner()
            .write_searcher_contract_info(searcher_contract, searcher_info)
    }

    fn write_builder_info(
        &self,
        builder_address: Address,
        builder_info: BuilderInfo,
    ) -> impl Future<Output = eyre::Result<()>> + Send {
        self.inner()
            .write_builder_info(builder_address, builder_info)
    }

    fn write_searcher_stats(
        &self,
        searcher_eoa: Address,
        searcher_stats: SearcherStats,
    ) -> impl Future<Output = eyre::Result<()>> + Send {
        self.inner()
            .write_searcher_stats(searcher_eoa, searcher_stats)
    }

    fn write_builder_stats(
        &self,
        builder_address: Address,
        builder_stats: BuilderStats,
    ) -> impl Future<Output = eyre::Result<()>> + Send {
        self.inner()
            .write_builder_stats(builder_address, builder_stats)
    }

    fn insert_pool(
        &self,
        block: u64,
        address: Address,
        tokens: &[Address],
        curve_lp_token: Option<Address>,
        classifier_name: Protocol,
    ) -> impl Future<Output = eyre::Result<()>> + Send {
        self.inner()
            .insert_pool(block, address, tokens, curve_lp_token, classifier_name)
    }

    fn save_traces(
        &self,
        block: u64,
        traces: Vec<TxTrace>,
    ) -> impl Future<Output = eyre::Result<()>> + Send {
        self.inner().save_traces(block, traces)
    }
}

pub struct NoopWriter;
impl DBWriter for NoopWriter {
    type Inner = Self;

    fn inner(&self) -> &Self::Inner {
        unreachable!();
    }

    async fn write_dex_quotes(
        &self,
        block_number: u64,
        quotes: Option<DexQuotes>,
    ) -> eyre::Result<()> {
        Ok(())
    }

    async fn write_token_info(
        &self,
        address: Address,
        decimals: u8,
        symbol: String,
    ) -> eyre::Result<()> {
        Ok(())
    }

    async fn save_mev_blocks(
        &self,
        block_number: u64,
        block: MevBlock,
        mev: Vec<Bundle>,
    ) -> eyre::Result<()> {
        Ok(())
    }

    async fn write_searcher_eoa_info(
        &self,
        searcher_eoa: Address,
        searcher_info: SearcherInfo,
    ) -> eyre::Result<()> {
        Ok(())
    }

    async fn write_searcher_contract_info(
        &self,
        searcher_contract: Address,
        searcher_info: SearcherInfo,
    ) -> eyre::Result<()> {
        Ok(())
    }

    async fn write_builder_info(
        &self,
        builder_coinbase_addr: Address,
        builder_info: BuilderInfo,
    ) -> eyre::Result<()> {
        Ok(())
    }

    async fn insert_pool(
        &self,
        block: u64,
        address: Address,
        tokens: &[Address],
        curve_lp_token: Option<Address>,
        classifier_name: Protocol,
    ) -> eyre::Result<()> {
        Ok(())
    }

    async fn save_traces(&self, block: u64, traces: Vec<TxTrace>) -> eyre::Result<()> {
        Ok(())
    }
}
