use std::{
    collections::HashMap,
    fmt::Debug,
    path,
    sync::{Arc, Mutex},
};

use ::clickhouse::DbRow;
use alloy_primitives::Address;
use brontes_types::{
    db::{
        builder::BuilderInfo,
        searcher::SearcherInfo,
        traits::{DBWriter, LibmdbxReader},
    },
    traits::TracingProvider,
    unordered_buffer_map::BrontesStreamExt,
    Protocol,
};
use futures::{future::join_all, stream::iter, StreamExt};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use toml::Table;
use tracing::{error, info};

use super::tables::Tables;
use crate::{
    clickhouse::ClickhouseHandle,
    libmdbx::{types::CompressedTable, LibmdbxData, LibmdbxReadWriter},
};
const CLASSIFIER_CONFIG_FILE_NAME: &str = "config/classifier_config.toml";
const SEARCHER_BUILDER_CONFIG_FILE_NAME: &str = "config/searcher_builder_config.toml";
const DEFAULT_START_BLOCK: u64 = 0;

pub struct LibmdbxInitializer<TP: TracingProvider, CH: ClickhouseHandle> {
    pub(crate) libmdbx: &'static LibmdbxReadWriter,
    clickhouse:         &'static CH,
    tracer:             Arc<TP>,
}

impl<TP: TracingProvider, CH: ClickhouseHandle> LibmdbxInitializer<TP, CH> {
    pub fn new(
        libmdbx: &'static LibmdbxReadWriter,
        clickhouse: &'static CH,
        tracer: Arc<TP>,
    ) -> Self {
        Self { libmdbx, clickhouse, tracer }
    }

    pub async fn initialize(
        &self,
        tables: &[Tables],
        clear_tables: bool,
        block_range: Option<(u64, u64)>, // inclusive of start only
    ) -> eyre::Result<()> {
        join_all(
            tables
                .iter()
                .map(|table| table.initialize_table(self, block_range, clear_tables)),
        )
        .await
        .into_iter()
        .collect::<eyre::Result<_>>()?;

        self.load_classifier_config_data().await;
        self.load_searcher_builder_config_data().await;
        Ok(())
    }

    pub async fn initialize_arbitrary_state(
        &self,
        tables: &[Tables],
        block_range: &'static [u64],
    ) -> eyre::Result<()> {
        join_all(
            tables
                .iter()
                .map(|table| table.initialize_table_arbitrary_state(self, block_range)),
        )
        .await
        .into_iter()
        .collect::<eyre::Result<_>>()?;

        self.load_classifier_config_data().await;
        self.load_searcher_builder_config_data().await;
        Ok(())
    }

    pub(crate) async fn clickhouse_init_no_args<'db, T, D>(
        &'db self,
        clear_table: bool,
    ) -> eyre::Result<()>
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
        if clear_table {
            self.libmdbx.0.clear_table::<T>()?;
        }

        let data = self.clickhouse.query_many::<T, D>().await;

        match data {
            Ok(d) => self.libmdbx.0.write_table(&d)?,
            Err(e) => {
                error!(target: "brontes::init", error=%e, "error initing {}", T::NAME)
            }
        }

        Ok(())
    }

    pub(crate) async fn initialize_table_from_clickhouse<T, D>(
        &self,
        block_range: Option<(u64, u64)>,
        clear_table: bool,
        mark_init: Option<u8>,
    ) -> eyre::Result<()>
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
        if clear_table {
            self.libmdbx.0.clear_table::<T>()?;
        }

        let block_range_chunks = if let Some((s, e)) = block_range {
            (s..e + 1).chunks(T::INIT_CHUNK_SIZE.unwrap_or((e - s + 1) as usize))
        } else {
            #[cfg(feature = "local-reth")]
            let end_block = self.tracer.best_block_number()?;
            #[cfg(not(feature = "local-reth"))]
            let end_block = self.tracer.best_block_number().await?;

            (DEFAULT_START_BLOCK..end_block + 1).chunks(
                T::INIT_CHUNK_SIZE.unwrap_or((end_block - DEFAULT_START_BLOCK + 1) as usize),
            )
        };

        let pair_ranges = block_range_chunks
            .into_iter()
            .map(|chk| chk.into_iter().collect_vec())
            .filter_map(
                |chk| {
                    if !chk.is_empty() {
                        Some((chk[0], chk[chk.len() - 1]))
                    } else {
                        None
                    }
                },
            )
            .collect_vec();

        let num_chunks = Arc::new(Mutex::new(pair_ranges.len()));

        info!(target: "brontes::init", "{} -- Starting Initialization With {} Chunks", T::NAME, pair_ranges.len());

        iter(pair_ranges.into_iter().map(|(start, end)| {
            let num_chunks = num_chunks.clone();
            let clickhouse = self.clickhouse;
            let libmdbx = self.libmdbx;

            async move {
                let data = clickhouse.query_many_range::<T, D>(start, end + 1).await;

                match data {
                    Ok(d) => {
                        libmdbx.0.write_table(&d)?;
                    }
                    Err(e) => {
                        info!(target: "brontes::init", "{} -- Error Writing -- {:?}", T::NAME, e)
                    }
                }

                let num = {
                    let mut n = num_chunks.lock().unwrap();
                    *n -= 1;
                    *n + 1
                };

                info!(target: "brontes::init", "{} -- Finished Chunk {}", T::NAME, num);
                if let Some(flag) = mark_init {
                    libmdbx.inited_range(start..=end, flag)?;
                }

                Ok::<(), eyre::Report>(())
            }
        }))
        .unordered_buffer_map(4, tokio::spawn)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;

        Ok(())
    }

    pub(crate) async fn initialize_table_from_clickhouse_arbitrary_state<'db, T, D>(
        &self,
        block_range: &'static [u64],
    ) -> eyre::Result<()>
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
        let ranges = block_range.chunks(T::INIT_CHUNK_SIZE.unwrap_or(1000000) / 100);

        let num_chunks = Arc::new(Mutex::new(ranges.len()));

        info!(target: "brontes::init::missing_state", "{} -- Starting Initialization Missing State With {} Chunks", T::NAME, ranges.len());

        iter(ranges.into_iter().map(|inner_range| {
            let num_chunks = num_chunks.clone();
            let clickhouse = self.clickhouse;
            let libmdbx = self.libmdbx;

            async move {
                let data = clickhouse.query_many_arbitrary::<T, D>(inner_range).await;

                match data {
                    Ok(d) => {
                        tracing::info!(target: "brontes::init::missing_state", "writing data of len {} to libmdbx", d.len());
                        libmdbx.0.write_table(&d)?;
                    }
                    Err(e) => {
                        info!(target: "brontes::init::missing_state", "{} -- Error Writing -- {:?}", T::NAME,  e)
                    }
                }

                let num = {
                    let mut n = num_chunks.lock().unwrap();
                    *n -= 1;
                    *n + 1
                };

                info!(target: "brontes::init::missing_state", "{} -- Finished Chunk {}", T::NAME, num);

                Ok::<(), eyre::Report>(())
            }
        }))
        .unordered_buffer_map(4, tokio::spawn)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;

        Ok(())
    }

    /// loads up the `classifier_config.toml` and ensures the values are in the
    /// database
    async fn load_classifier_config_data(&self) {
        let mut workspace_dir = workspace_dir();
        workspace_dir.push(CLASSIFIER_CONFIG_FILE_NAME);

        let Ok(config) = toml::from_str::<Table>(&{
            let Ok(path) = std::fs::read_to_string(workspace_dir) else {
                tracing::error!("failed to read classifier_config");
                return;
            };
            path
        }) else {
            tracing::error!("failed to load toml");
            return;
        };

        for (protocol, inner) in config {
            let protocol: Protocol = protocol.parse().unwrap();
            for (address, table) in inner.as_table().unwrap() {
                let token_addr: Address = address.parse().unwrap();
                let init_block = table.get("init_block").unwrap().as_integer().unwrap() as u64;

                let table: Vec<TokenInfoWithAddressToml> = table
                    .get("token_info")
                    .map(|i| i.clone().try_into())
                    .unwrap_or(Ok(vec![]))
                    .unwrap_or(vec![]);

                for t_info in &table {
                    self.libmdbx
                        .write_token_info(t_info.address, t_info.decimals, t_info.symbol.clone())
                        .await
                        .unwrap();
                }

                let token_addrs = if table.len() < 2 {
                    [Address::default(), Address::default()]
                } else {
                    [table[0].address, table[1].address]
                };

                self.libmdbx
                    .insert_pool(init_block, token_addr, &token_addrs, None, protocol)
                    .await
                    .unwrap();
            }
        }
    }

    async fn load_searcher_builder_config_data(&self) {
        let mut workspace_dir = workspace_dir();
        workspace_dir.push(SEARCHER_BUILDER_CONFIG_FILE_NAME);

        let config_str =
            std::fs::read_to_string(workspace_dir).expect("Failed to read config file");

        let config: BSConfig = toml::from_str(&config_str).expect("Failed to parse TOML");

        // Process builders
        for (address_str, builder_info) in config.builders {
            let address = address_str.parse().unwrap();

            let existing_info = self.libmdbx.try_fetch_builder_info(address);

            match existing_info.expect("Failed to query builder table") {
                Some(mut existing) => {
                    existing.merge(builder_info);
                    self.libmdbx
                        .write_builder_info(address, existing) // Assuming this method exists
                        .await
                        .expect("Failed to update builder info");
                }
                None => {
                    self.libmdbx
                        .write_builder_info(address, builder_info)
                        .await
                        .expect("Failed to write new builder info");
                }
            }
        }

        // Process SearcherEOAs
        for (address_str, searcher_info) in config.searcher_eoas {
            let address = address_str.parse().unwrap();
            let existing_info = self.libmdbx.try_fetch_searcher_eoa_info(address);

            match existing_info.expect("Failed to query builder table") {
                Some(mut existing) => {
                    existing.merge(searcher_info);
                    self.libmdbx
                        .write_searcher_eoa_info(address, existing) // Assuming this method exists
                        .await
                        .expect("Failed to update searcher info");
                }
                None => {
                    self.libmdbx
                        .write_searcher_eoa_info(address, searcher_info)
                        .await
                        .expect("Failed to write new builder info");
                }
            }
        }
        // Process SearcherContracts
        for (address_str, searcher_info) in config.searcher_contracts {
            let address = address_str.parse().unwrap();
            let existing_info = self.libmdbx.try_fetch_searcher_contract_info(address);

            match existing_info.expect("Failed to query builder table") {
                Some(mut existing) => {
                    existing.merge(searcher_info);
                    self.libmdbx
                        .write_searcher_contract_info(address, existing) // Assuming this method exists
                        .await
                        .expect("Failed to update searcher info");
                }
                None => {
                    self.libmdbx
                        .write_searcher_contract_info(address, searcher_info)
                        .await
                        .expect("Failed to write new builder info");
                }
            }
        }
    }
}

fn workspace_dir() -> path::PathBuf {
    let output = std::process::Command::new(env!("CARGO"))
        .arg("locate-project")
        .arg("--workspace")
        .arg("--message-format=plain")
        .output()
        .unwrap()
        .stdout;
    let cargo_path = path::Path::new(std::str::from_utf8(&output).unwrap().trim());
    cargo_path.parent().unwrap().to_path_buf()
}

#[derive(Debug, Deserialize, Default)]
pub struct TokenInfoWithAddressToml {
    pub symbol:   String,
    pub decimals: u8,
    pub address:  Address,
}

#[derive(Serialize, Deserialize, Debug)]
struct BSConfig {
    builders:           HashMap<String, BuilderInfo>,
    searcher_eoas:      HashMap<String, SearcherInfo>,
    searcher_contracts: HashMap<String, SearcherInfo>,
}

#[cfg(test)]
mod tests {
    use brontes_core::test_utils::{get_db_handle, init_trace_parser};
    use brontes_database::libmdbx::{
        initialize::LibmdbxInitializer, tables::*, test_utils::load_clickhouse,
    };
    use tokio::sync::mpsc::unbounded_channel;

    #[brontes_macros::test]
    async fn test_intialize_clickhouse_tables() {
        //let block_range = (17000000, 17000100);
        let block_range = (17000000, 17000002);
        let arbitrary_set = Box::leak(Box::new(vec![17000000, 17000010, 17000100]));

        let clickhouse = Box::leak(Box::new(load_clickhouse().await));
        let libmdbx = get_db_handle(tokio::runtime::Handle::current().clone()).await;
        let (tx, _rx) = unbounded_channel();
        let tracing_client =
            init_trace_parser(tokio::runtime::Handle::current().clone(), tx, libmdbx, 4).await;

        let intializer = LibmdbxInitializer::new(libmdbx, clickhouse, tracing_client.get_tracer());

        let tables = Tables::ALL;

        intializer
            .initialize(&tables, false, Some(block_range))
            .await
            .unwrap();

        // TokenDecimals
        TokenDecimals::test_initialized_data(clickhouse, libmdbx, None)
            .await
            .unwrap();

        // AddressToProtocol
        AddressToProtocolInfo::test_initialized_data(clickhouse, libmdbx, None)
            .await
            .unwrap();

        // CexPrice
        CexPrice::test_initialized_data(clickhouse, libmdbx, Some(block_range))
            .await
            .unwrap();
        CexPrice::test_initialized_arbitrary_data(clickhouse, libmdbx, arbitrary_set)
            .await
            .unwrap();

        // Metadata
        BlockInfo::test_initialized_data(clickhouse, libmdbx, Some(block_range))
            .await
            .unwrap();
        BlockInfo::test_initialized_arbitrary_data(clickhouse, libmdbx, arbitrary_set)
            .await
            .unwrap();

        // PoolCreationBlocks
        PoolCreationBlocks::test_initialized_data(clickhouse, libmdbx, None)
            .await
            .unwrap();

        // Builder
        Builder::test_initialized_data(clickhouse, libmdbx, None)
            .await
            .unwrap();

        // AddressMeta
        AddressMeta::test_initialized_data(clickhouse, libmdbx, None)
            .await
            .unwrap();

        // TxTraces
        TxTraces::test_initialized_data(clickhouse, libmdbx, Some(block_range))
            .await
            .unwrap();
        TxTraces::test_initialized_arbitrary_data(clickhouse, libmdbx, arbitrary_set)
            .await
            .unwrap();
    }
}
