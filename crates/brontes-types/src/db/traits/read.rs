use alloy_primitives::Address;

use crate::{
    db::{
        address_metadata::AddressMetadata, address_to_protocol_info::ProtocolInfo,
        builder::BuilderInfo, cex::trades::CexTradeMap, dex::DexQuotes, metadata::Metadata,
        mev_block::MevBlockWithClassified, searcher::SearcherInfo,
        token_info::TokenInfoWithAddress,
    },
    pair::Pair,
    structured_trace::TxTrace,
    FastHashMap, Protocol,
};
pub type AllSearcherInfo = (Vec<(Address, SearcherInfo)>, Vec<(Address, SearcherInfo)>);
pub type ProtocolCreatedRange = FastHashMap<u64, Vec<(Address, Protocol, Pair)>>;

#[auto_impl::auto_impl(&, Box)]
pub trait LibmdbxReader: Send + Sync + Unpin + 'static {
    fn get_most_recent_block(&self) -> eyre::Result<u64>;
    fn get_metadata_no_dex_price(
        &self,
        block_num: u64,
        quote_asset: Address,
    ) -> eyre::Result<Metadata>;

    fn has_dex_quotes(&self, block_num: u64) -> eyre::Result<bool>;

    fn try_fetch_searcher_info(
        &self,
        eoa_address: Address,
        contract_address: Option<Address>,
    ) -> eyre::Result<(Option<SearcherInfo>, Option<SearcherInfo>)> {
        let eoa_info = self.try_fetch_searcher_eoa_info(eoa_address)?;

        if let Some(contract_address) = contract_address {
            let contract_info = self.try_fetch_searcher_contract_info(contract_address)?;
            Ok((eoa_info, contract_info))
        } else {
            Ok((eoa_info, None))
        }
    }

    fn try_fetch_searcher_infos(
        &self,
        eoa: Vec<Address>,
        contract: Vec<Address>,
    ) -> eyre::Result<FastHashMap<Address, (SearcherInfo, Option<SearcherInfo>)>> {
        let eoa_info = self.try_fetch_searcher_eoa_infos(eoa)?;
        let mut contract = self.try_fetch_searcher_contract_infos(contract)?;
        Ok(eoa_info
            .into_iter()
            .map(|(k, v)| (k, (v, contract.remove(&k))))
            .collect())
    }

    fn try_fetch_address_metadatas(
        &self,
        addresses: Vec<Address>,
    ) -> eyre::Result<FastHashMap<Address, AddressMetadata>>;

    fn fetch_all_searcher_info(&self) -> eyre::Result<AllSearcherInfo> {
        let eoa_info = self.fetch_all_searcher_eoa_info()?;
        let contract_info = self.fetch_all_searcher_contract_info()?;

        Ok((eoa_info, contract_info))
    }

    fn fetch_all_searcher_eoa_info(&self) -> eyre::Result<Vec<(Address, SearcherInfo)>>;

    fn fetch_all_searcher_contract_info(&self) -> eyre::Result<Vec<(Address, SearcherInfo)>>;

    fn try_fetch_searcher_eoa_info(
        &self,
        searcher_eoa: Address,
    ) -> eyre::Result<Option<SearcherInfo>>;

    fn try_fetch_searcher_contract_info(
        &self,
        searcher_contract: Address,
    ) -> eyre::Result<Option<SearcherInfo>>;

    fn try_fetch_searcher_eoa_infos(
        &self,
        searcher_eoa: Vec<Address>,
    ) -> eyre::Result<FastHashMap<Address, SearcherInfo>>;

    fn try_fetch_searcher_contract_infos(
        &self,
        searcher_contract: Vec<Address>,
    ) -> eyre::Result<FastHashMap<Address, SearcherInfo>>;

    fn try_fetch_builder_info(
        &self,
        builder_coinbase_addr: Address,
    ) -> eyre::Result<Option<BuilderInfo>>;

    fn fetch_all_builder_info(&self) -> eyre::Result<Vec<(Address, BuilderInfo)>>;

    fn get_metadata(&self, block_num: u64, quote_asset: Address) -> eyre::Result<Metadata>;

    fn get_cex_trades(&self, block: u64) -> eyre::Result<CexTradeMap>;

    fn try_fetch_address_metadata(&self, address: Address)
        -> eyre::Result<Option<AddressMetadata>>;

    fn fetch_all_address_metadata(&self) -> eyre::Result<Vec<(Address, AddressMetadata)>>;

    fn get_dex_quotes(&self, block: u64) -> eyre::Result<DexQuotes>;

    fn try_fetch_token_info(&self, address: Address) -> eyre::Result<TokenInfoWithAddress>;

    fn try_fetch_token_decimals(&self, address: Address) -> eyre::Result<u8> {
        self.try_fetch_token_info(address).map(|info| info.decimals)
    }

    fn try_fetch_mev_blocks(
        &self,
        start_block: Option<u64>,
        end_block: u64,
    ) -> eyre::Result<Vec<MevBlockWithClassified>>;

    fn fetch_all_mev_blocks(
        &self,
        start_block: Option<u64>,
    ) -> eyre::Result<Vec<MevBlockWithClassified>>;

    fn protocols_created_before(
        &self,
        start_block: u64,
    ) -> eyre::Result<FastHashMap<(Address, Protocol), Pair>>;

    fn protocols_created_range(
        &self,
        start_block: u64,
        end_block: u64,
    ) -> eyre::Result<ProtocolCreatedRange>;

    fn get_protocol(&self, address: Address) -> eyre::Result<Protocol> {
        self.get_protocol_details(address).map(|res| res.protocol)
    }

    fn get_protocol_details(&self, address: Address) -> eyre::Result<ProtocolInfo>;

    /// returns protocol details with the tokens sorted from smallest to
    /// biggest. This is needed as for some reason the tokens in the
    /// database for a given protocol don't seems to always be ordered
    /// correctly
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
