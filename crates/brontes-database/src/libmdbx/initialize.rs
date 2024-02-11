use std::{
    fmt::Debug,
    sync::{Arc, Mutex},
};

use brontes_types::{traits::TracingProvider, unordered_buffer_map::BrontesStreamExt};
use futures::{future::join_all, stream::iter, StreamExt};
use itertools::Itertools;
use serde::Deserialize;
use sorella_db_databases::{clickhouse::DbRow, Database};
use tracing::{error, info};

use super::tables::Tables;
use crate::{
    clickhouse::Clickhouse,
    libmdbx::{types::CompressedTable, LibmdbxData, LibmdbxReadWriter},
};

const DEFAULT_START_BLOCK: u64 = 0;

pub struct LibmdbxInitializer<TP: TracingProvider> {
    pub(crate) libmdbx: &'static LibmdbxReadWriter,
    clickhouse:         &'static Clickhouse,
    tracer:             Arc<TP>,
}

impl<TP: TracingProvider> LibmdbxInitializer<TP> {
    pub fn new(
        libmdbx: &'static LibmdbxReadWriter,
        clickhouse: &'static Clickhouse,
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
        .collect::<eyre::Result<_>>()
    }

    pub(crate) async fn clickhouse_init_no_args<'db, T, D>(
        &'db self,
        clear_table: bool,
    ) -> eyre::Result<()>
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
        D: LibmdbxData<T> + DbRow + for<'de> Deserialize<'de> + Send + Sync + Debug + 'static,
    {
        if clear_table {
            self.libmdbx.0.clear_table::<T>()?;
        }

        let data = self
            .clickhouse
            .inner()
            .query_many::<D>(
                T::INIT_QUERY.expect("Should only be called on clickhouse tables"),
                &(),
            )
            .await;

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
        D: LibmdbxData<T> + DbRow + for<'de> Deserialize<'de> + Send + Sync + Debug + 'static,
    {
        if clear_table {
            self.libmdbx.0.clear_table::<T>()?;
        }

        let block_range_chunks = if let Some((s, e)) = block_range {
            (s..e + 1).chunks(T::INIT_CHUNK_SIZE.unwrap_or((e - s + 1) as usize))
        } else {
            #[cfg(not(feature = "local"))]
            let end_block = self.tracer.best_block_number()?;
            #[cfg(feature = "local")]
            let end_block = self.tracer.best_block_number().await?;

            (DEFAULT_START_BLOCK..end_block + 1).chunks(
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

        let num_chunks = Arc::new(Mutex::new(pair_ranges.len()));

        info!(target: "brontes::init", "{} -- Starting Initialization With {} Chunks", T::NAME, pair_ranges.len());

        iter(pair_ranges.into_iter().map(|(start, end)| {
            let num_chunks = num_chunks.clone();
            let clickhouse = self.clickhouse;
            let libmdbx = self.libmdbx;

            async move {
                let data = clickhouse
                    .inner()
                    .query_many::<D>(
                        T::INIT_QUERY.expect("Should only be called on clickhouse tables"),
                        &(start, end + 1),
                    )
                    .await;

                match data {
                    Ok(d) => libmdbx.0.write_table(&d)?,
                    Err(e) => {
                        info!(target: "brontes::init", "{} -- Error Writing -- {:?}", T::NAME,  e)
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
}

#[cfg(test)]
mod tests {
    use brontes_core::test_utils::{get_db_handle, init_trace_parser, init_tracing};
    use brontes_database::libmdbx::{
        initialize::LibmdbxInitializer, tables::*, test_utils::init_clickhouse,
    };
    use tokio::sync::mpsc::unbounded_channel;

    #[brontes_macros::test]
    async fn test_intialize_clickhouse_no_args_tables() {
        init_tracing();
        let block_range = (17000000, 17000100);

        let clickhouse = Box::leak(Box::new(init_clickhouse()));
        let libmdbx = get_db_handle();
        let (tx, _rx) = unbounded_channel();
        let tracing_client =
            init_trace_parser(tokio::runtime::Handle::current().clone(), tx, libmdbx, 4);

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
    }
}
