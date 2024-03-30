use std::sync::Arc;

use alloy_primitives::Address;
use brontes_types::{
    db::{
        address_metadata::AddressMetadata,
        address_to_protocol_info::ProtocolInfo,
        builder::BuilderInfo,
        dex::DexQuotes,
        metadata::Metadata,
        mev_block::MevBlockWithClassified,
        searcher::SearcherInfo,
        token_info::TokenInfoWithAddress,
        traits::{DBWriter, LibmdbxReader, ProtocolCreatedRange},
    },
    mev::{Bundle, MevBlock},
    normalized_actions::Actions,
    pair::Pair,
    structured_trace::TxTrace,
    BlockTree, FastHashMap, Protocol, SubGraphEdge,
};

use super::Clickhouse;
use crate::{clickhouse::ClickhouseHandle, libmdbx::LibmdbxInit};

pub struct ClickhouseMiddleware<I: DBWriter> {
    #[allow(dead_code)] // on tests feature
    client: Clickhouse,
    inner:  I,
}

impl<I: DBWriter> ClickhouseMiddleware<I> {
    pub fn new(client: Clickhouse, inner: I) -> Self {
        Self { inner, client }
    }
}

impl<I: DBWriter + Send + Sync> DBWriter for ClickhouseMiddleware<I> {
    type Inner = I;

    fn inner(&self) -> &Self::Inner {
        &self.inner
    }

    async fn write_dex_quotes(
        &self,
        block_number: u64,
        quotes: Option<DexQuotes>,
    ) -> eyre::Result<()> {
        self.client
            .write_dex_quotes(block_number, quotes.clone())
            .await?;

        self.inner().write_dex_quotes(block_number, quotes).await
    }

    async fn write_token_info(
        &self,
        address: Address,
        decimals: u8,
        symbol: String,
    ) -> eyre::Result<()> {
        self.client
            .write_token_info(address, decimals, symbol.clone())
            .await?;

        self.inner()
            .write_token_info(address, decimals, symbol)
            .await
    }

    async fn save_mev_blocks(
        &self,
        block_number: u64,
        block: MevBlock,
        mev: Vec<Bundle>,
    ) -> eyre::Result<()> {
        self.client
            .save_mev_blocks(block_number, block.clone(), mev.clone())
            .await?;

        self.inner().save_mev_blocks(block_number, block, mev).await
    }

    async fn write_searcher_eoa_info(
        &self,
        searcher_eoa: Address,
        searcher_info: SearcherInfo,
    ) -> eyre::Result<()> {
        self.client
            .write_searcher_eoa_info(searcher_eoa, searcher_info.clone())
            .await?;

        self.inner()
            .write_searcher_eoa_info(searcher_eoa, searcher_info)
            .await
    }

    async fn write_searcher_contract_info(
        &self,
        searcher_contract: Address,
        searcher_info: SearcherInfo,
    ) -> eyre::Result<()> {
        self.client
            .write_searcher_contract_info(searcher_contract, searcher_info.clone())
            .await?;

        self.inner()
            .write_searcher_contract_info(searcher_contract, searcher_info)
            .await
    }

    async fn write_builder_info(
        &self,
        builder_coinbase_addr: Address,
        builder_info: BuilderInfo,
    ) -> eyre::Result<()> {
        self.client
            .write_builder_info(builder_coinbase_addr, builder_info.clone())
            .await?;

        self.inner()
            .write_builder_info(builder_coinbase_addr, builder_info)
            .await
    }

    async fn insert_pool(
        &self,
        block: u64,
        address: Address,
        tokens: &[Address],
        curve_lp_token: Option<Address>,
        classifier_name: Protocol,
    ) -> eyre::Result<()> {
        self.client
            .insert_pool(block, address, tokens, curve_lp_token, classifier_name)
            .await?;

        self.inner()
            .insert_pool(block, address, tokens, curve_lp_token, classifier_name)
            .await
    }

    async fn insert_tree(&self, tree: Arc<BlockTree<Actions>>) -> eyre::Result<()> {
        self.client.insert_tree(tree.clone()).await?;

        self.inner().insert_tree(tree).await?;

        Ok(())
    }

    async fn save_traces(&self, block: u64, traces: Vec<TxTrace>) -> eyre::Result<()> {
        self.client.save_traces(block, traces.clone()).await?;

        self.inner().save_traces(block, traces).await
    }
}

impl<I: LibmdbxInit> LibmdbxInit for ClickhouseMiddleware<I> {
    async fn initialize_tables<T: brontes_types::traits::TracingProvider, CH: ClickhouseHandle>(
        &'static self,
        clickhouse: &'static CH,
        tracer: std::sync::Arc<T>,
        tables: &[crate::Tables],
        clear_tables: bool,
        block_range: Option<(u64, u64)>, // inclusive of start only
    ) -> eyre::Result<()> {
        self.inner
            .initialize_tables(clickhouse, tracer, tables, clear_tables, block_range)
            .await
    }

    async fn initialize_tables_arbitrary<
        T: brontes_types::traits::TracingProvider,
        CH: ClickhouseHandle,
    >(
        &'static self,
        clickhouse: &'static CH,
        tracer: std::sync::Arc<T>,
        tables: &[crate::Tables],
        block_range: Vec<u64>,
    ) -> eyre::Result<()> {
        self.inner
            .initialize_tables_arbitrary(clickhouse, tracer, tables, block_range)
            .await
    }

    async fn init_full_range_tables<CH: ClickhouseHandle>(&self, clickhouse: &'static CH) -> bool {
        self.inner.init_full_range_tables(clickhouse).await
    }

    fn state_to_initialize(
        &self,
        start_block: u64,
        end_block: u64,
    ) -> eyre::Result<Vec<std::ops::RangeInclusive<u64>>> {
        self.inner.state_to_initialize(start_block, end_block)
    }
}

impl<I: LibmdbxInit> LibmdbxReader for ClickhouseMiddleware<I> {
    fn get_metadata_no_dex_price(&self, block_num: u64) -> eyre::Result<Metadata> {
        self.inner.get_metadata_no_dex_price(block_num)
    }

    fn try_fetch_searcher_eoa_info(
        &self,
        searcher_eoa: Address,
    ) -> eyre::Result<Option<SearcherInfo>> {
        self.inner.try_fetch_searcher_eoa_info(searcher_eoa)
    }

    fn try_fetch_searcher_contract_info(
        &self,
        searcher_eoa: Address,
    ) -> eyre::Result<Option<SearcherInfo>> {
        self.inner.try_fetch_searcher_contract_info(searcher_eoa)
    }

    fn fetch_all_searcher_eoa_info(&self) -> eyre::Result<Vec<(Address, SearcherInfo)>> {
        self.inner.fetch_all_searcher_eoa_info()
    }

    fn fetch_all_searcher_contract_info(&self) -> eyre::Result<Vec<(Address, SearcherInfo)>> {
        self.inner.fetch_all_searcher_contract_info()
    }

    fn fetch_all_searcher_info(
        &self,
    ) -> eyre::Result<(Vec<(Address, SearcherInfo)>, Vec<(Address, SearcherInfo)>)> {
        self.inner.fetch_all_searcher_info()
    }

    fn try_fetch_builder_info(
        &self,
        builder_coinbase_addr: Address,
    ) -> eyre::Result<Option<BuilderInfo>> {
        self.inner.try_fetch_builder_info(builder_coinbase_addr)
    }

    fn fetch_all_builder_info(&self) -> eyre::Result<Vec<(Address, BuilderInfo)>> {
        self.inner.fetch_all_builder_info()
    }

    //TODO: JOE
    fn try_fetch_mev_blocks(
        &self,
        _start_block: Option<u64>,
        _end_block: u64,
    ) -> eyre::Result<Vec<MevBlockWithClassified>> {
        todo!("Joe");
    }

    fn fetch_all_mev_blocks(
        &self,
        _start_block: Option<u64>,
    ) -> eyre::Result<Vec<MevBlockWithClassified>> {
        todo!("Joe");
    }

    fn get_metadata(&self, block_num: u64) -> eyre::Result<Metadata> {
        self.inner.get_metadata(block_num)
    }

    fn try_fetch_address_metadata(
        &self,
        address: Address,
    ) -> eyre::Result<Option<AddressMetadata>> {
        self.inner.try_fetch_address_metadata(address)
    }

    fn fetch_all_address_metadata(&self) -> eyre::Result<Vec<(Address, AddressMetadata)>> {
        self.inner.fetch_all_address_metadata()
    }

    fn get_dex_quotes(&self, block: u64) -> eyre::Result<DexQuotes> {
        self.inner.get_dex_quotes(block)
    }

    fn try_fetch_token_info(&self, address: Address) -> eyre::Result<TokenInfoWithAddress> {
        self.inner.try_fetch_token_info(address)
    }

    fn protocols_created_before(
        &self,
        start_block: u64,
    ) -> eyre::Result<FastHashMap<(Address, Protocol), Pair>> {
        self.inner.protocols_created_before(start_block)
    }

    fn protocols_created_range(
        &self,
        start_block: u64,
        end_block: u64,
    ) -> eyre::Result<ProtocolCreatedRange> {
        self.inner.protocols_created_range(start_block, end_block)
    }

    fn try_load_pair_before(
        &self,
        block: u64,
        pair: Pair,
    ) -> eyre::Result<(Pair, Vec<SubGraphEdge>)> {
        self.inner.try_load_pair_before(block, pair)
    }

    fn get_protocol_details(&self, address: Address) -> eyre::Result<ProtocolInfo> {
        self.inner.get_protocol_details(address)
    }

    fn load_trace(&self, block_num: u64) -> eyre::Result<Vec<TxTrace>> {
        self.inner.load_trace(block_num)
    }
}

pub struct ReadOnlyMiddleware<I: DBWriter> {
    #[allow(dead_code)] // on tests feature
    pub client: Clickhouse,
    inner:      I,
}

impl<I: DBWriter> ReadOnlyMiddleware<I> {
    pub fn new(client: Clickhouse, inner: I) -> Self {
        Self { inner, client }
    }
}

impl<I: DBWriter + Send + Sync> DBWriter for ReadOnlyMiddleware<I> {
    type Inner = Self;

    fn inner(&self) -> &Self::Inner {
        self
    }

    async fn write_dex_quotes(
        &self,
        block_number: u64,
        quotes: Option<DexQuotes>,
    ) -> eyre::Result<()> {
        self.client
            .write_dex_quotes(block_number, quotes.clone())
            .await
    }

    async fn write_token_info(
        &self,
        address: Address,
        decimals: u8,
        symbol: String,
    ) -> eyre::Result<()> {
        self.client
            .write_token_info(address, decimals, symbol.clone())
            .await
    }

    async fn save_mev_blocks(
        &self,
        block_number: u64,
        block: MevBlock,
        mev: Vec<Bundle>,
    ) -> eyre::Result<()> {
        self.client
            .save_mev_blocks(block_number, block.clone(), mev.clone())
            .await
    }

    async fn write_searcher_eoa_info(
        &self,
        searcher_eoa: Address,
        searcher_info: SearcherInfo,
    ) -> eyre::Result<()> {
        self.client
            .write_searcher_eoa_info(searcher_eoa, searcher_info.clone())
            .await
    }

    async fn write_searcher_contract_info(
        &self,
        searcher_contract: Address,
        searcher_info: SearcherInfo,
    ) -> eyre::Result<()> {
        self.client
            .write_searcher_contract_info(searcher_contract, searcher_info.clone())
            .await
    }

    async fn write_builder_info(
        &self,
        builder_coinbase_addr: Address,
        builder_info: BuilderInfo,
    ) -> eyre::Result<()> {
        self.client
            .write_builder_info(builder_coinbase_addr, builder_info.clone())
            .await
    }

    async fn insert_pool(
        &self,
        block: u64,
        address: Address,
        tokens: &[Address],
        curve_lp_token: Option<Address>,
        classifier_name: Protocol,
    ) -> eyre::Result<()> {
        self.client
            .insert_pool(block, address, tokens, curve_lp_token, classifier_name)
            .await
    }

    async fn insert_tree(&self, tree: Arc<BlockTree<Actions>>) -> eyre::Result<()> {
        self.client.insert_tree(tree.clone()).await?;

        Ok(())
    }

    async fn save_traces(&self, block: u64, traces: Vec<TxTrace>) -> eyre::Result<()> {
        self.client.save_traces(block, traces.clone()).await
    }
}

impl<I: LibmdbxInit> LibmdbxInit for ReadOnlyMiddleware<I> {
    async fn initialize_tables<T: brontes_types::traits::TracingProvider, CH: ClickhouseHandle>(
        &'static self,
        clickhouse: &'static CH,
        tracer: std::sync::Arc<T>,
        tables: &[crate::Tables],
        clear_tables: bool,
        block_range: Option<(u64, u64)>, // inclusive of start only
    ) -> eyre::Result<()> {
        self.inner
            .initialize_tables(clickhouse, tracer, tables, clear_tables, block_range)
            .await
    }

    async fn initialize_tables_arbitrary<
        T: brontes_types::traits::TracingProvider,
        CH: ClickhouseHandle,
    >(
        &'static self,
        clickhouse: &'static CH,
        tracer: std::sync::Arc<T>,
        tables: &[crate::Tables],
        block_range: Vec<u64>,
    ) -> eyre::Result<()> {
        self.inner
            .initialize_tables_arbitrary(clickhouse, tracer, tables, block_range)
            .await
    }

    async fn init_full_range_tables<CH: ClickhouseHandle>(&self, clickhouse: &'static CH) -> bool {
        self.inner.init_full_range_tables(clickhouse).await
    }

    fn state_to_initialize(
        &self,
        start_block: u64,
        end_block: u64,
    ) -> eyre::Result<Vec<std::ops::RangeInclusive<u64>>> {
        self.inner.state_to_initialize(start_block, end_block)
    }
}

impl<I: LibmdbxInit> LibmdbxReader for ReadOnlyMiddleware<I> {
    fn get_metadata_no_dex_price(&self, block_num: u64) -> eyre::Result<Metadata> {
        self.inner.get_metadata_no_dex_price(block_num)
    }

    fn fetch_all_searcher_eoa_info(&self) -> eyre::Result<Vec<(Address, SearcherInfo)>> {
        self.inner.fetch_all_searcher_eoa_info()
    }

    fn fetch_all_searcher_contract_info(&self) -> eyre::Result<Vec<(Address, SearcherInfo)>> {
        self.inner.fetch_all_searcher_contract_info()
    }

    fn try_fetch_searcher_eoa_info(
        &self,
        searcher_eoa: Address,
    ) -> eyre::Result<Option<SearcherInfo>> {
        self.inner.try_fetch_searcher_eoa_info(searcher_eoa)
    }

    fn try_fetch_searcher_contract_info(
        &self,
        searcher_eoa: Address,
    ) -> eyre::Result<Option<SearcherInfo>> {
        self.inner.try_fetch_searcher_contract_info(searcher_eoa)
    }

    fn try_fetch_builder_info(
        &self,
        builder_coinbase_addr: Address,
    ) -> eyre::Result<Option<BuilderInfo>> {
        self.inner.try_fetch_builder_info(builder_coinbase_addr)
    }

    fn fetch_all_builder_info(&self) -> eyre::Result<Vec<(Address, BuilderInfo)>> {
        self.inner.fetch_all_builder_info()
    }

    //TODO: JOE
    fn try_fetch_mev_blocks(
        &self,
        _start_block: Option<u64>,
        _end_block: u64,
    ) -> eyre::Result<Vec<MevBlockWithClassified>> {
        todo!("Joe");
    }

    fn fetch_all_mev_blocks(
        &self,
        _start_block: Option<u64>,
    ) -> eyre::Result<Vec<MevBlockWithClassified>> {
        todo!("Joe");
    }

    fn get_metadata(&self, block_num: u64) -> eyre::Result<Metadata> {
        self.inner.get_metadata(block_num)
    }

    fn try_fetch_address_metadata(
        &self,
        address: Address,
    ) -> eyre::Result<Option<AddressMetadata>> {
        self.inner.try_fetch_address_metadata(address)
    }

    fn fetch_all_address_metadata(&self) -> eyre::Result<Vec<(Address, AddressMetadata)>> {
        self.inner.fetch_all_address_metadata()
    }

    fn get_dex_quotes(&self, block: u64) -> eyre::Result<DexQuotes> {
        self.inner.get_dex_quotes(block)
    }

    fn try_fetch_token_info(&self, address: Address) -> eyre::Result<TokenInfoWithAddress> {
        self.inner.try_fetch_token_info(address)
    }

    fn protocols_created_before(
        &self,
        start_block: u64,
    ) -> eyre::Result<FastHashMap<(Address, Protocol), Pair>> {
        self.inner.protocols_created_before(start_block)
    }

    fn protocols_created_range(
        &self,
        start_block: u64,
        end_block: u64,
    ) -> eyre::Result<ProtocolCreatedRange> {
        self.inner.protocols_created_range(start_block, end_block)
    }

    fn try_load_pair_before(
        &self,
        block: u64,
        pair: Pair,
    ) -> eyre::Result<(Pair, Vec<SubGraphEdge>)> {
        self.inner.try_load_pair_before(block, pair)
    }

    fn get_protocol_details(&self, address: Address) -> eyre::Result<ProtocolInfo> {
        self.inner.get_protocol_details(address)
    }

    fn load_trace(&self, block_num: u64) -> eyre::Result<Vec<TxTrace>> {
        self.inner.load_trace(block_num)
    }
}
