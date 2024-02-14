use alloy_primitives::Address;
use brontes_types::{
    db::{dex::DexQuotes, searcher::SearcherInfo, traits::DBWriter},
    mev::{Bundle, MevBlock},
    structured_trace::TxTrace,
    Protocol,
};

use super::Clickhouse;

pub struct ClickhouseMiddleware<I: DBWriter> {
    client: Clickhouse,
    inner: I,
}

impl<I: DBWriter> DBWriter for ClickhouseMiddleware<I> {
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

    async fn write_searcher_info(
        &self,
        searcher_eoa: Address,
        searcher_info: SearcherInfo,
    ) -> eyre::Result<()> {
        self.client
            .write_searcher_info(searcher_eoa, searcher_info.clone())
            .await?;

        self.inner()
            .write_searcher_info(searcher_eoa, searcher_info)
            .await
    }

    async fn insert_pool(
        &self,
        block: u64,
        address: Address,
        tokens: [Address; 2],
        classifier_name: Protocol,
    ) -> eyre::Result<()> {
        self.client
            .insert_pool(block, address, tokens, classifier_name)
            .await?;

        self.inner()
            .insert_pool(block, address, tokens, classifier_name)
            .await
    }

    async fn save_traces(&self, block: u64, traces: Vec<TxTrace>) -> eyre::Result<()> {
        self.client.save_traces(block, traces.clone()).await?;

        self.inner().save_traces(block, traces).await
    }
}
