use alloy_primitives::Address;
use futures::Future;

use crate::{
    db::{
        address_metadata::AddressMetadata, builder::BuilderInfo, dex::DexQuotes,
        searcher::SearcherInfo,
    },
    mev::{Bundle, MevBlock},
    normalized_actions::Action,
    structured_trace::TxTrace,
    BlockTree, Protocol,
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

    fn write_address_meta(
        &self,
        address: Address,
        metadata: AddressMetadata,
    ) -> impl Future<Output = eyre::Result<()>> + Send {
        self.inner().write_address_meta(address, metadata)
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

    fn insert_tree(
        &self,
        tree: BlockTree<Action>,
    ) -> impl Future<Output = eyre::Result<()>> + Send {
        self.inner().insert_tree(tree)
    }

    fn save_traces(
        &self,
        block: u64,
        traces: Vec<TxTrace>,
    ) -> impl Future<Output = eyre::Result<()>> + Send {
        self.inner().save_traces(block, traces)
    }
}
