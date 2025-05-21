use std::fmt::Debug;

use alloy_primitives::{Address, TxHash};
use brontes_types::{
    db::{
        dex::{DexPrices, DexQuotes},
        metadata::{BlockMetadata, Metadata},
    },
    pair::Pair,
    FastHashMap,
};
use clickhouse::{remote_cursor::RemoteCursor, DbRow};
use futures::TryStreamExt;
use itertools::Itertools;
use reqwest::StatusCode;
use reth_primitives::BlockHash;
use serde::Deserialize;

use crate::{
    clickhouse::ClickhouseHandle,
    libmdbx::{cex_utils::CexRangeOrArbitrary, determine_eth_prices, types::LibmdbxData},
    BlockInfo, BlockInfoData, CexPrice, CexPriceData, CompressedTable, DexPrice, DexPriceData,
};

pub struct ClickhouseHttpClient {
    client:  reqwest::Client,
    url:     String,
    api_key: String,
}

impl ClickhouseHttpClient {
    pub async fn new(url: String, api_key: Option<String>) -> Self {
        let client = reqwest::Client::new();
        let api_key = if let Some(key) = api_key {
            key
        } else {
            let resp = client
                .get(format!("{}/register", url))
                .send()
                .await
                .unwrap();
            if resp.status() == StatusCode::OK {
                resp.text().await.unwrap()
            } else {
                let text = resp.text().await.unwrap();
                text.split("key: ").collect_vec()[1].to_string()
            }
        };
        Self { url, api_key, client }
    }

    fn process_dex_quotes(val: DexPriceData) -> DexQuotes {
        let mut dex_quotes: Vec<Option<FastHashMap<Pair, DexPrices>>> = Vec::new();
        let dex_q = val.value;
        for _ in dex_quotes.len()..=dex_q.tx_idx as usize {
            dex_quotes.push(None);
        }

        let tx = dex_quotes.get_mut(dex_q.tx_idx as usize).unwrap();

        if let Some(tx) = tx.as_mut() {
            for (pair, price) in dex_q.quote {
                tx.insert(pair, price);
            }
        } else {
            let mut tx_pairs = FastHashMap::default();
            for (pair, price) in dex_q.quote {
                tx_pairs.insert(pair, price);
            }
            *tx = Some(tx_pairs);
        }
        DexQuotes(dex_quotes)
    }
}

impl ClickhouseHandle for ClickhouseHttpClient {
    async fn get_metadata(
        &self,
        block_num: u64,
        _: u64,
        _: BlockHash,
        _: Vec<TxHash>,
        quote_asset: Address,
        include_relay: bool,
    ) -> eyre::Result<Metadata> {
        let block_meta = self
            .query_many_range::<BlockInfo, BlockInfoData>(block_num, block_num + 1)
            .await?
            .pop()
            .ok_or_else(|| eyre::eyre!("no block data found"))?;

        let cex_quotes = self
            .query_many_range::<CexPrice, CexPriceData>(block_num, block_num + 1)
            .await
            .unwrap_or_default()
            .pop()
            .unwrap_or_default();

        let dex_quotes = self
            .query_many_range::<DexPrice, DexPriceData>(block_num, block_num + 1)
            .await
            .map(|mut e| e.pop())
            .ok()
            .flatten()
            .map(Self::process_dex_quotes);

        let eth_price = determine_eth_prices(
            &cex_quotes.value,
            block_meta.value.block_timestamp * 1_000_000,
            quote_asset,
        );

        Ok({
            let metadata = BlockMetadata::new(
                block_num,
                block_meta.value.block_hash,
                block_meta.value.block_timestamp,
                block_meta.value.relay_timestamp,
                block_meta.value.p2p_timestamp,
                block_meta.value.proposer_fee_recipient,
                block_meta.value.proposer_mev_reward,
                eth_price.unwrap_or_default(),
                block_meta.value.private_flow.into_iter().collect(),
            );
            metadata.into_metadata(cex_quotes.value, dex_quotes, None, None)
        })
    }

    async fn query_many_range<T, D>(&self, start_block: u64, end_block: u64) -> eyre::Result<Vec<D>>
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
        D: LibmdbxData<T>
            + DbRow
            + for<'de> Deserialize<'de>
            + Send
            + Sync
            + Debug
            + Unpin
            + 'static,
    {
        let request = self
            .client
            .get(format!(
                "{}/{}",
                self.url,
                T::HTTP_ENDPOINT.unwrap_or_else(|| panic!(
                    "tried to init remote when no http endpoint was set {}",
                    T::NAME
                ))
            ))
            .header("api-key", &self.api_key)
            .header("start-block", start_block)
            .header("end-block", end_block)
            .build()?;

        tracing::debug!(?request, "querying endpoint");

        let mut cur = RemoteCursor::new(
            self.client
                .execute(request)
                .await
                .inspect_err(|e| {
                    if let Some(status_code) = e.status() {
                        tracing::error!(%status_code, "clickhouse http query")
                    }
                })?
                .bytes_stream(),
        );
        let mut res = Vec::new();
        while let Some(next) = cur.try_next().await? {
            res.push(next)
        }

        Ok(res)
    }

    async fn query_many<T, D>(&self) -> eyre::Result<Vec<D>>
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
        D: LibmdbxData<T>
            + DbRow
            + for<'de> Deserialize<'de>
            + Send
            + Sync
            + Debug
            + Unpin
            + 'static,
    {
        let mut cur = RemoteCursor::new(
            self.client
                .get(format!(
                    "{}/{}",
                    self.url,
                    T::HTTP_ENDPOINT.unwrap_or_else(|| panic!(
                        "tried to init remote when no http endpoint was set {}",
                        T::NAME
                    ))
                ))
                .header("api-key", &self.api_key)
                .send()
                .await?
                .bytes_stream(),
        );

        let mut res = Vec::new();
        while let Some(next) = cur.try_next().await? {
            res.push(next)
        }

        Ok(res)
    }

    async fn query_many_arbitrary<T, D>(&self, range: &'static [u64]) -> eyre::Result<Vec<D>>
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
        D: LibmdbxData<T>
            + DbRow
            + for<'de> Deserialize<'de>
            + Send
            + Sync
            + Debug
            + Unpin
            + 'static,
    {
        let range_str = range
            .iter()
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(",");

        let request = self
            .client
            .get(format!(
                "{}/{}",
                self.url,
                T::HTTP_ENDPOINT.unwrap_or_else(|| panic!(
                    "tried to init remote when no http endpoint was set {}",
                    T::NAME
                ))
            ))
            .header("api-key", &self.api_key)
            .header("block-set", range_str)
            .build()?;

        tracing::debug!(?request, "querying endpoint");

        let mut cur = RemoteCursor::new(
            self.client
                .execute(request)
                .await
                .inspect_err(|e| {
                    if let Some(status_code) = e.status() {
                        tracing::error!(%status_code, "clickhouse http query")
                    }
                })?
                .bytes_stream(),
        );

        let mut res = Vec::new();
        while let Some(next) = cur.try_next().await? {
            res.push(next)
        }

        Ok(res)
    }

    async fn get_cex_prices(
        &self,
        _range_or_arbitrary: CexRangeOrArbitrary,
    ) -> eyre::Result<Vec<crate::CexPriceData>> {
        unimplemented!()
    }

    async fn get_cex_trades(
        &self,
        _range_or_arbitrary: CexRangeOrArbitrary,
    ) -> eyre::Result<Vec<crate::CexTradesData>> {
        unimplemented!()
    }
}

#[cfg(test)]
pub mod test {

    use brontes_types::constants::USDT_ADDRESS;

    use crate::{clickhouse::ClickhouseHandle, libmdbx::test_utils::load_clickhouse};

    #[brontes_macros::test]
    async fn test_metadata_query() {
        let click_house = load_clickhouse().await;
        let res = click_house
            .get_metadata(
                18500000,
                Default::default(),
                Default::default(),
                Default::default(),
                USDT_ADDRESS,
            )
            .await;

        assert!(res.is_ok());
    }
}
