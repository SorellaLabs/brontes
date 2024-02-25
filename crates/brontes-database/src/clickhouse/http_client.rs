use std::{cmp::max, collections::HashMap, fmt::Debug};

use brontes_types::{
    db::{
        dex::{DexPrices, DexQuotes},
        metadata::{BlockMetadata, Metadata},
    },
    pair::Pair,
};
use clickhouse::DbRow;
use serde::Deserialize;

use crate::{
    clickhouse::ClickhouseHandle,
    libmdbx::{determine_eth_prices, types::LibmdbxData},
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
            client
                .get(format!("{}/register", url))
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap()
        };
        Self { url, api_key, client }
    }

    fn process_dex_quotes(val: DexPriceData) -> DexQuotes {
        let mut dex_quotes: Vec<Option<HashMap<Pair, DexPrices>>> = Vec::new();
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
            let mut tx_pairs = HashMap::default();
            for (pair, price) in dex_q.quote {
                tx_pairs.insert(pair, price);
            }
            *tx = Some(tx_pairs);
        }
        DexQuotes(dex_quotes)
    }
}

impl ClickhouseHandle for ClickhouseHttpClient {
    async fn get_metadata(&self, block_num: u64) -> eyre::Result<Metadata> {
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

        let eth_prices = determine_eth_prices(&cex_quotes.value);

        Ok({
            BlockMetadata::new(
                block_num,
                block_meta.value.block_hash,
                block_meta.value.block_timestamp,
                block_meta.value.relay_timestamp,
                block_meta.value.p2p_timestamp,
                block_meta.value.proposer_fee_recipient,
                block_meta.value.proposer_mev_reward,
                max(eth_prices.price.0, eth_prices.price.1),
                block_meta.value.private_flow.into_iter().collect(),
            )
            .into_metadata(cex_quotes.value, dex_quotes, None)
        })
    }

    async fn query_many_range<T, D>(&self, start_block: u64, end_block: u64) -> eyre::Result<Vec<D>>
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
        D: LibmdbxData<T> + DbRow + for<'de> Deserialize<'de> + Send + Sync + Debug + 'static,
    {
        self.client
            .get(format!(
                "{}/{}",
                self.url,
                T::HTTP_ENDPOINT.expect("tried to init remote when no http endpoint was set")
            ))
            .header("api-key", &self.api_key)
            .header("start-block", start_block)
            .header("end-block", end_block)
            .send()
            .await?
            .json()
            .await
            .map_err(Into::into)
    }

    async fn query_many<T, D>(&self) -> eyre::Result<Vec<D>>
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
        D: LibmdbxData<T> + DbRow + for<'de> Deserialize<'de> + Send + Sync + Debug + 'static,
    {
        self.client
            .get(format!(
                "{}/{}",
                self.url,
                T::HTTP_ENDPOINT.expect("tried to init remote when no http endpoint was set")
            ))
            .header("api-key", &self.api_key)
            .send()
            .await?
            .json()
            .await
            .map_err(Into::into)
    }
}

#[cfg(test)]
pub mod test {

    use crate::{clickhouse::ClickhouseHandle, libmdbx::test_utils::load_clickhouse};

    #[brontes_macros::test]
    async fn test_metadata_query() {
        let click_house = load_clickhouse().await;
        let res = click_house.get_metadata(18500000).await;
        assert!(res.is_ok());
    }
}
