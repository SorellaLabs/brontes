use std::{fmt::Debug, path, sync::Arc};

use ::clickhouse::DbRow;
use alloy_primitives::Address;
use brontes_types::{
    db::{
        address_metadata::{AddressMetadata, ContractInfo, Socials},
        builder::BuilderInfo,
        searcher::SearcherInfo,
        traits::{DBWriter, LibmdbxReader},
    },
    traits::TracingProvider,
    unordered_buffer_map::BrontesStreamExt,
    FastHashMap, Protocol,
};
use futures::{join, stream::iter, StreamExt};
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressState, ProgressStyle};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use toml::Table as tomlTable;
use tracing::{error, info};

use super::tables::Tables;
use crate::{
    clickhouse::ClickhouseHandle,
    libmdbx::{
        cex_utils::CexRangeOrArbitrary, types::CompressedTable, LibmdbxData, LibmdbxReadWriter,
    },
};
const CLASSIFIER_CONFIG_FILE: &str = "config/classifier_config.toml";
const SEARCHER_CONFIG_FILE: &str = "config/searcher_config.toml";
const BUILDER_CONFIG_FILE: &str = "config/builder_config.toml";
const METADATA_CONFIG_FILE: &str = "config/metadata_config.toml";
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
        table: Tables,
        clear_tables: bool,
        block_range: Option<(u64, u64)>, // inclusive of start only
        progress_bar: Arc<Vec<(Tables, ProgressBar)>>,
    ) -> eyre::Result<()> {
        table
            .initialize_table(self, block_range, clear_tables, progress_bar)
            .await
    }

    pub async fn initialize_arbitrary_state(
        &self,
        table: Tables,
        block_range: &'static [u64],
        progress_bar: Arc<Vec<(Tables, ProgressBar)>>,
    ) -> eyre::Result<()> {
        table
            .initialize_table_arbitrary_state(self, block_range, progress_bar.clone())
            .await
    }

    pub async fn initialize_full_range_tables(&self) -> eyre::Result<()> {
        let tables = &[
            Tables::PoolCreationBlocks,
            Tables::AddressToProtocolInfo,
            Tables::TokenDecimals,
            Tables::Builder,
            Tables::AddressMeta,
        ];
        let progress_bar = Self::build_critical_state_progress_bar(5).unwrap();

        futures::stream::iter(tables.to_vec())
            .map(|table| {
                let progress_bar = progress_bar.clone();
                async move { table.initialize_full_range_table(self, progress_bar).await }
            })
            .buffer_unordered(5)
            .collect::<Vec<_>>()
            .await;

        Ok(())
    }

    pub async fn load_config(&self) -> eyre::Result<()> {
        join!(
            self.load_classifier_config_data(),
            self.load_searcher_config_data(),
            self.load_builder_config_data(),
            self.load_address_metadata_config(),
        );

        Ok(())
    }

    pub(crate) async fn clickhouse_init_no_args<'db, T, D>(
        &'db self,
        clear_table: bool,
        progress_bar: ProgressBar,
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
            Ok(d) => {
                progress_bar.inc(1);
                self.libmdbx.0.write_table(&d)?
            }
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
        cex_table_flag: bool,

        pb: ProgressBar,
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
            (s..=e).chunks(T::INIT_CHUNK_SIZE.unwrap_or((e - s + 1) as usize))
        } else {
            #[cfg(feature = "local-reth")]
            let end_block = self.tracer.best_block_number()?;
            #[cfg(not(feature = "local-reth"))]
            let end_block = self.tracer.best_block_number().await?;

            (DEFAULT_START_BLOCK..=end_block).chunks(
                T::INIT_CHUNK_SIZE.unwrap_or((end_block - DEFAULT_START_BLOCK + 1) as usize),
            )
        };

        let pair_ranges = block_range_chunks
            .into_iter()
            .map(|chk| chk.into_iter().collect_vec())
            .filter_map(
                |chk| if !chk.is_empty() { Some((chk[0], chk[chk.len() - 1])) } else { None },
            )
            .collect_vec();

        iter(pair_ranges.into_iter().map(|(start, end)| {
            let clickhouse = self.clickhouse;
            let libmdbx = self.libmdbx;
            let pb = pb.clone();
            let count = end - start;

            async move {

                if cex_table_flag {
                    #[cfg(not(feature = "cex-dex-markout"))]
                    {
                        let data = clickhouse
                            .get_cex_prices(CexRangeOrArbitrary::Range(start, end+1))
                            .await;
                        match data {
                            Ok(d) => {
                                pb.inc(count);
                                libmdbx.0.write_table(&d)?;
                            }
                            Err(e) => {
                                error!(target: "brontes::init", "{} -- Error Writing -- {:?}", T::NAME, e)
                            }
                        }
                    }


                    #[cfg(feature = "cex-dex-markout")]
                    {
                        let data = clickhouse
                            .get_cex_trades(CexRangeOrArbitrary::Range(start, end+1))
                            .await;
                        match data {
                            Ok(d) => {
                                pb.inc(count);
                                libmdbx.0.write_table(&d)?;
                            }
                            Err(e) => {
                                error!(target: "brontes::init", "{} -- Error Writing -- {:?}", T::NAME, e)
                            }
                        }
                    }

                } else {
                    let data = clickhouse
                    .query_many_range::<T, D>(start, end + 1)
                    .await;
                    match data {
                        Ok(d) => {
                            pb.inc(count);
                            libmdbx.0.write_table(&d)?;
                        }
                        Err(e) => {
                            info!(target: "brontes::init", "{} -- Error Writing -- {:?}", T::NAME, e)
                        }
                    }
                }

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
        mark_init: Option<u8>,
        cex_table_flag: bool,
        pb: ProgressBar,
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
        let ranges = block_range.chunks(T::INIT_CHUNK_SIZE.unwrap_or(1000));

        iter(ranges.into_iter().map(|inner_range| {
            let clickhouse = self.clickhouse;
            let libmdbx = self.libmdbx;
            let pb = pb.clone();
            let count = inner_range.len() as u64;

            async move {

                if cex_table_flag {
                    #[cfg(not(feature = "cex-dex-markout"))]
                    {
                        let data = clickhouse
                            .get_cex_prices(CexRangeOrArbitrary::Arbitrary(inner_range))
                            .await;
                        match data {
                            Ok(d) => {
                                pb.inc(count);
                                libmdbx.0.write_table(&d)?;
                            }
                            Err(e) => {
                                info!(target: "brontes::init", "{} -- Error Writing -- {:?}", T::NAME, e)
                            }
                        }
                    }


                    #[cfg(feature = "cex-dex-markout")]
                    {
                        let data = clickhouse
                        .get_cex_trades(CexRangeOrArbitrary::Arbitrary(inner_range))
                        .await;
                        match data {
                            Ok(d) => {
                                pb.inc(count);
                                libmdbx.0.write_table(&d)?;
                            }
                            Err(e) => {
                                info!(target: "brontes::init", "{} -- Error Writing -- {:?}", T::NAME, e)
                            }
                        }
                    }

                } else {
                    let data = clickhouse
                        .query_many_arbitrary::<T, D>(inner_range).await;
                    match data {
                        Ok(d) => {
                            pb.inc(count);
                            libmdbx.0.write_table(&d)?;
                        }
                        Err(e) => {
                            info!(target: "brontes::init", "{} -- Error Writing -- {:?}", T::NAME, e)
                        }
                    }
                }

                if let Some(flag) = mark_init {
                    libmdbx.inited_range_arbitrary(inner_range.iter().copied(), flag)?;
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

    fn build_critical_state_progress_bar(table_count: u64) -> Option<ProgressBar> {
        if table_count == 0 {
            return None
        }

        let progress_bar =
            ProgressBar::with_draw_target(Some(table_count), ProgressDrawTarget::stderr_with_hz(5));
        progress_bar.set_style(
            ProgressStyle::with_template(
                "{msg}\n[{elapsed_precise}] [{wide_bar:.green/red}] {pos}/{len} ({percent}%)",
            )
            .unwrap()
            .progress_chars("█>-")
            .with_key(
                "percent",
                |state: &ProgressState, f: &mut dyn std::fmt::Write| {
                    write!(f, "{:.1}", state.fraction() * 100.0).unwrap()
                },
            ),
        );
        progress_bar.set_message("Critical Tables Init:");

        Some(progress_bar)
    }

    /// loads up the `classifier_config.toml` and ensures the values are in the
    /// database
    async fn load_classifier_config_data(&self) {
        let mut workspace_dir = workspace_dir();
        workspace_dir.push(CLASSIFIER_CONFIG_FILE);

        let Ok(config) = toml::from_str::<tomlTable>(&{
            let Ok(path) = std::fs::read_to_string(workspace_dir) else {
                tracing::error!(target: "brontes::init", "failed to read classifier_config");
                return;
            };
            path
        }) else {
            tracing::error!(target: "brontes::init", "failed to load toml");
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

    async fn load_builder_config_data(&self) {
        let mut builder_config_path = workspace_dir();
        builder_config_path.push(BUILDER_CONFIG_FILE);

        let builder_config_str = std::fs::read_to_string(builder_config_path)
            .expect("Failed to read builder config file");

        let builder_config: BuilderConfig =
            toml::from_str(&builder_config_str).expect("Failed to parse builder TOML");

        // Process builders
        for (address_str, builder_info) in builder_config.builders {
            let address: Address = address_str
                .parse()
                .unwrap_or_else(|_| panic!("Failed to parse address '{}'", address_str));
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
    }

    async fn load_searcher_config_data(&self) {
        let mut searcher_config_path = workspace_dir();

        searcher_config_path.push(SEARCHER_CONFIG_FILE);

        let searcher_config_str = std::fs::read_to_string(searcher_config_path)
            .expect("Failed to read searcher config file");

        let searcher_config: SearcherConfig =
            toml::from_str(&searcher_config_str).expect("Failed to parse searcher TOML");

        // Process SearcherEOAs
        for (address_str, searcher_info) in searcher_config.searcher_eoas {
            let address = address_str
                .parse()
                .unwrap_or_else(|_| panic!("Failed to parse address '{}'", address_str));

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
        for (address_str, searcher_info) in searcher_config.searcher_contracts {
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

    async fn load_address_metadata_config(&self) {
        let mut workspace_dir = workspace_dir();
        workspace_dir.push(METADATA_CONFIG_FILE);

        let config_str =
            std::fs::read_to_string(workspace_dir).expect("Failed to read config file");

        let config: MetadataConfig = toml::from_str(&config_str).expect("Failed to parse TOML");

        for (address_str, toml_metadata) in config.metadata {
            let address = address_str.parse().unwrap();
            let metadata: AddressMetadata = toml_metadata.into_address_metadata();

            let existing_info = self.libmdbx.try_fetch_address_metadata(address);

            match existing_info.expect("Failed to query address metadata table") {
                Some(mut existing) => {
                    existing.merge(metadata);
                    self.libmdbx
                        .write_address_meta(address, existing)
                        .await
                        .expect("Failed to write address metadata");
                }
                None => {
                    self.libmdbx
                        .write_address_meta(address, metadata)
                        .await
                        .expect("Failed to write address metadata");
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
struct BuilderConfig {
    builders: FastHashMap<String, BuilderInfo>,
}

#[derive(Serialize, Deserialize, Debug)]
struct SearcherConfig {
    searcher_eoas:      FastHashMap<String, SearcherInfo>,
    searcher_contracts: FastHashMap<String, SearcherInfo>,
}

#[derive(Serialize, Deserialize)]
pub struct AddressMetadataConfig {
    pub entity_name:     Option<String>,
    pub nametag:         Option<String>,
    pub labels:          Option<Vec<String>>,
    #[serde(rename = "type")]
    pub address_type:    Option<String>,
    #[serde(default)]
    pub contract_info:   Option<ContractInfoConfig>,
    pub ens:             Option<String>,
    pub social_metadata: Option<SocialsConfig>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct ContractInfoConfig {
    pub verified_contract: Option<bool>,
    pub contract_creator:  Option<String>,
    pub reputation:        Option<u8>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct SocialsConfig {
    pub twitter:           Option<String>,
    pub twitter_followers: Option<u64>,
    pub website_url:       Option<String>,
    pub crunchbase:        Option<String>,
    pub linkedin:          Option<String>,
}
#[derive(Serialize, Deserialize)]
struct MetadataConfig {
    pub metadata: FastHashMap<String, AddressMetadataConfig>,
}

impl AddressMetadataConfig {
    fn into_address_metadata(self) -> AddressMetadata {
        AddressMetadata {
            entity_name:     self.entity_name,
            nametag:         self.nametag,
            labels:          self.labels.unwrap_or_default(),
            address_type:    self.address_type,
            contract_info:   self.contract_info.map(|config| ContractInfo {
                verified_contract: config.verified_contract,
                contract_creator:  config.contract_creator.map(|s| s.parse().unwrap()),
                reputation:        config.reputation,
            }),
            ens:             self.ens,
            social_metadata: self
                .social_metadata
                .map(|config| Socials {
                    twitter:           config.twitter,
                    twitter_followers: config.twitter_followers,
                    website_url:       config.website_url,
                    crunchbase:        config.crunchbase,
                    linkedin:          config.linkedin,
                })
                .unwrap_or_default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use brontes_core::test_utils::{get_db_handle, init_trace_parser};
    use brontes_database::libmdbx::{
        initialize::LibmdbxInitializer, tables::*, test_utils::load_clickhouse,
    };
    use brontes_types::init_threadpools;
    use indicatif::MultiProgress;
    use itertools::Itertools;
    use tokio::sync::mpsc::unbounded_channel;

    #[brontes_macros::test]
    async fn test_intialize_clickhouse_tables() {
        let block_range = (19000000, 19000002);

        let clickhouse = Box::leak(Box::new(load_clickhouse().await));
        init_threadpools(10);
        let libmdbx = get_db_handle(tokio::runtime::Handle::current().clone()).await;
        let (tx, _rx) = unbounded_channel();
        let tracing_client =
            init_trace_parser(tokio::runtime::Handle::current().clone(), tx, libmdbx, 4).await;

        let intializer = LibmdbxInitializer::new(libmdbx, clickhouse, tracing_client.get_tracer());

        let tables = Tables::ALL;

        let multi = MultiProgress::default();
        let tables_cnt = Arc::new(
            Tables::ALL
                .into_iter()
                .map(|table| (table, table.build_init_state_progress_bar(&multi, 69)))
                .collect_vec(),
        );

        for table in tables {
            intializer
                .initialize(table, false, Some(block_range), tables_cnt.clone())
                .await
                .unwrap();
        }

        // TokenDecimals
        TokenDecimals::test_initialized_data(clickhouse, libmdbx, None)
            .await
            .unwrap();

        // AddressToProtocol
        AddressToProtocolInfo::test_initialized_data(clickhouse, libmdbx, None)
            .await
            .unwrap();

        // Metadata
        BlockInfo::test_initialized_data(clickhouse, libmdbx, Some(block_range))
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
    }
}
