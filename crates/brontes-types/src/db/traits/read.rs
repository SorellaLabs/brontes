use std::collections::HashMap;

use alloy_primitives::Address;

use crate::{
    db::{
        address_metadata::AddressMetadata, address_to_tokens::PoolTokens, builder::BuilderInfo,
        dex::DexQuotes, metadata::Metadata, searcher::SearcherInfo,
        token_info::TokenInfoWithAddress,
    },
    pair::Pair,
    structured_trace::TxTrace,
    Protocol, SubGraphEdge,
};

#[auto_impl::auto_impl(&)]
pub trait LibmdbxReader: Send + Sync + Unpin + 'static {
    fn get_metadata_no_dex_price(&self, block_num: u64) -> eyre::Result<Metadata>;

    fn try_fetch_searcher_info(&self, searcher_eoa: Address) -> eyre::Result<Option<SearcherInfo>>;

    fn try_fetch_builder_info(
        &self,
        builder_coinbase_addr: Address,
    ) -> eyre::Result<Option<BuilderInfo>>;

    fn get_metadata(&self, block_num: u64) -> eyre::Result<Metadata>;

    fn try_fetch_address_metadata(&self, address: Address)
        -> eyre::Result<Option<AddressMetadata>>;
    fn get_dex_quotes(&self, block: u64) -> eyre::Result<DexQuotes>;

    fn try_fetch_token_info(&self, address: Address) -> eyre::Result<Option<TokenInfoWithAddress>>;

    fn try_fetch_token_decimals(&self, address: Address) -> eyre::Result<Option<u8>> {
        Ok(self
            .try_fetch_token_info(address)?
            .map(|info| info.decimals))
    }

    fn protocols_created_before(
        &self,
        start_block: u64,
    ) -> eyre::Result<HashMap<(Address, Protocol), Pair>>;

    fn protocols_created_range(
        &self,
        start_block: u64,
        end_block: u64,
    ) -> eyre::Result<HashMap<u64, Vec<(Address, Protocol, Pair)>>>;

    fn try_load_pair_before(
        &self,
        block: u64,
        pair: Pair,
    ) -> eyre::Result<(Pair, Vec<SubGraphEdge>)>;

    fn get_protocol_tokens(&self, address: Address) -> eyre::Result<Option<PoolTokens>>;
    fn get_protocol(&self, address: Address) -> eyre::Result<Option<Protocol>>;
    fn load_trace(&self, block_num: u64) -> eyre::Result<Option<Vec<TxTrace>>>;
}
