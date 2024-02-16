use std::collections::HashMap;

use alloy_primitives::Address;

use crate::{
    db::{
        address_metadata::AddressMetadata, address_to_protocol_info::ProtocolInfo,
        builder::BuilderInfo, dex::DexQuotes, metadata::Metadata,
        mev_block::MevBlockWithClassified, searcher::SearcherInfo,
        token_info::TokenInfoWithAddress,
    },
    pair::Pair,
    structured_trace::TxTrace,
    Protocol, SubGraphEdge,
};

pub type ProtocolCreatedRange = HashMap<u64, Vec<(Address, Protocol, Pair)>>;

#[auto_impl::auto_impl(&)]
pub trait LibmdbxReader: Send + Sync + Unpin + 'static {
    fn get_metadata_no_dex_price(&self, block_num: u64) -> eyre::Result<Metadata>;

    fn try_fetch_searcher_info(&self, searcher_eoa: Address) -> eyre::Result<SearcherInfo>;

    fn try_fetch_builder_info(&self, builder_coinbase_addr: Address) -> eyre::Result<BuilderInfo>;

    fn get_metadata(&self, block_num: u64) -> eyre::Result<Metadata>;

    fn try_fetch_address_metadata(&self, address: Address) -> eyre::Result<AddressMetadata>;

    fn get_dex_quotes(&self, block: u64) -> eyre::Result<DexQuotes>;

    fn try_fetch_token_info(&self, address: Address) -> eyre::Result<TokenInfoWithAddress>;

    fn try_fetch_token_decimals(&self, address: Address) -> eyre::Result<u8> {
        self.try_fetch_token_info(address).map(|info| info.decimals)
    }

    fn try_fetch_mev_blocks(
        &self,
        start_block: u64,
        end_block: u64,
    ) -> eyre::Result<Vec<MevBlockWithClassified>>;

    fn protocols_created_before(
        &self,
        start_block: u64,
    ) -> eyre::Result<HashMap<(Address, Protocol), Pair>>;

    fn protocols_created_range(
        &self,
        start_block: u64,
        end_block: u64,
    ) -> eyre::Result<ProtocolCreatedRange>;

    fn try_load_pair_before(
        &self,
        block: u64,
        pair: Pair,
    ) -> eyre::Result<(Pair, Vec<SubGraphEdge>)>;

    fn get_protocol(&self, address: Address) -> eyre::Result<Protocol> {
        self.get_protocol_details(address).map(|res| res.protocol)
    }

    fn get_protocol_details(&self, address: Address) -> eyre::Result<ProtocolInfo>;

    /// returns protocol details with the tokens sorted from smallest to biggest. This
    /// is needed as for some reason the tokens in the database for a given protocol don't
    /// seems to always be ordered correctly
    fn get_protocol_details_sorted(&self, address: Address) -> eyre::Result<ProtocolInfo> {
        self.get_protocol_details(address).map(|mut info| {
            if info.token0 > info.token1 {
                std::mem::swap(&mut info.token0, &mut info.token1)
            }
            info
        })
    }

    fn load_trace(&self, block_num: u64) -> eyre::Result<Vec<TxTrace>>;
}
