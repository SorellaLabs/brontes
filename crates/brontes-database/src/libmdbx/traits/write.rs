use alloy_primitives::Address;
use brontes_pricing::{Protocol, SubGraphEdge};
use brontes_types::{db::dex::DexQuotes, extra_processing::Pair, structured_trace::TxTrace};

pub trait LibmdbxWriter: Send + Sync + 'static {
    fn write_dex_quotes(&self, block_number: u64, quotes: DexQuotes) -> eyre::Result<()>;
    fn write_token_decimals(&self, address: Address, decimals: u8) -> eyre::Result<()>;
    fn save_pair_at(&self, block: u64, pair: Pair, edges: Vec<SubGraphEdge>) -> eyre::Result<()>;
    fn insert_pool(
        &self,
        block: u64,
        address: Address,
        tokens: [Address; 2],
        classifier_name: Protocol,
    ) -> eyre::Result<()>;

    fn save_traces(&self, block: u64, traces: Vec<TxTrace>) -> eyre::Result<()>;
}
