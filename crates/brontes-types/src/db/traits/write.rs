use alloy_primitives::Address;

use crate::{
    db::dex::DexQuotes,
    mev::{Bundle, MevBlock},
    pair::Pair,
    structured_trace::TxTrace,
    Protocol, SubGraphEdge,
};

#[auto_impl::auto_impl(&)]
pub trait LibmdbxWriter: Send + Sync + Unpin + 'static {
    fn write_dex_quotes(&self, block_number: u64, quotes: DexQuotes) -> eyre::Result<()>;
    fn write_token_info(&self, address: Address, decimals: u8, symbol: String) -> eyre::Result<()>;
    fn save_pair_at(&self, block: u64, pair: Pair, edges: Vec<SubGraphEdge>) -> eyre::Result<()>;
    fn save_mev_blocks(
        &self,
        block_number: u64,
        block: MevBlock,
        mev: Vec<Bundle>,
    ) -> eyre::Result<()>;

    fn insert_pool(
        &self,
        block: u64,
        address: Address,
        tokens: [Address; 2],
        classifier_name: Protocol,
    ) -> eyre::Result<()>;

    fn save_traces(&self, block: u64, traces: Vec<TxTrace>) -> eyre::Result<()>;
}
