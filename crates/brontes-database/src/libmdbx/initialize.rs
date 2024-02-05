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

use super::{tables::Tables, types::LibmdbxData, Libmdbx};
use crate::{clickhouse::Clickhouse, libmdbx::types::CompressedTable};

const DEFAULT_START_BLOCK: u64 = 0;

pub struct LibmdbxInitializer<TP: TracingProvider> {
    libmdbx:    Arc<Libmdbx>,
    clickhouse: Arc<Clickhouse>,
    tracer:     Arc<TP>,
}

impl<TP: TracingProvider> LibmdbxInitializer<TP> {
    pub fn new(libmdbx: Arc<Libmdbx>, clickhouse: Arc<Clickhouse>, tracer: Arc<TP>) -> Self {
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
                .map(|table| table.initialize_table(&self, block_range, clear_tables)),
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
            self.libmdbx.clear_table::<T>()?;
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
            Ok(d) => self.libmdbx.write_table(&d)?,
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
    ) -> eyre::Result<()>
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
        D: LibmdbxData<T> + DbRow + for<'de> Deserialize<'de> + Send + Sync + Debug + 'static,
    {
        if clear_table {
            self.libmdbx.clear_table::<T>()?;
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
                |chk| if chk.len() != 0 { Some((chk[0], chk[chk.len() - 1])) } else { None },
            )
            .collect_vec();

        let num_chunks = Arc::new(Mutex::new(pair_ranges.len()));

        info!(target: "brontes::init", "{} -- Starting Initialization With {} Chunks", T::NAME, pair_ranges.len());

        iter(pair_ranges.into_iter().map(|(start, end)| {
            let num_chunks = num_chunks.clone();
            let clickhouse = self.clickhouse.clone();
            let libmdbx = self.libmdbx.clone();

            async move {
                let data = clickhouse
                    .inner()
                    .query_many::<D>(
                        T::INIT_QUERY.expect("Should only be called on clickhouse tables"),
                        &(start, end),
                    )
                    .await;

                match data {
                    Ok(d) => libmdbx.write_table(&d)?,
                    Err(e) => {
                        info!(target: "brontes::init", "{} -- Error Writing -- {:?}", T::NAME,  e)
                    }
                }

                let num = {
                    let mut n = num_chunks.lock().unwrap();
                    *n -= 1;
                    n.clone() + 1
                };

                info!(target: "brontes::init", "{} -- Finished Chunk {}", T::NAME, num);

                Ok::<(), eyre::Report>(())
            }
        }))
        .unordered_buffer_map(4, |fut| tokio::spawn(fut))
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serial_test::serial;

    use self::test_utils::*;
    use super::LibmdbxInitializer;
    use crate::libmdbx::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 5)]
    #[serial]
    async fn test_intialize_clickhouse_no_args_tables() {
        let block_range = (17000000, 17000100);

        #[cfg(not(feature = "local"))]
        let tracing_client =
            Arc::new(init_tracing(tokio::runtime::Handle::current().clone()).unwrap());
        #[cfg(feature = "local")]
        let tracing_client = Arc::new(init_tracing().unwrap());

        let clickhouse = Arc::new(init_clickhouse());
        let libmdbx = Arc::new(init_libmdbx().unwrap());

        let intializer =
            LibmdbxInitializer::new(libmdbx.clone(), clickhouse.clone(), tracing_client);

        let tables = Tables::ALL;
        intializer
            .initialize(&tables, true, Some(block_range))
            .await
            .unwrap();

        // TokenDecimals
        let (c, l) = TokenDecimals::test_initialized_data(&clickhouse, &libmdbx, None)
            .await
            .unwrap();
        assert_eq!(c, l);

        // AddressToTokens
        let (c, l) = AddressToTokens::test_initialized_data(&clickhouse, &libmdbx, None)
            .await
            .unwrap();
        assert_eq!(c, l);

        // AddressToProtocol
        let (c, l) = AddressToProtocol::test_initialized_data(&clickhouse, &libmdbx, None)
            .await
            .unwrap();
        assert_eq!(c, l);

        // CexPrice
        let (c, l) = CexPrice::test_initialized_data(&clickhouse, &libmdbx, Some(block_range))
            .await
            .unwrap();
        assert_eq!(c, l);

        // Metadata
        let (c, l) = BlockInfo::test_initialized_data(&clickhouse, &libmdbx, Some(block_range))
            .await
            .unwrap();
        assert_eq!(c, l);

        // PoolCreationBlocks
        let (c, l) =
            PoolCreationBlocks::test_initialized_data(&clickhouse, &libmdbx, Some(block_range))
                .await
                .unwrap();
        assert_eq!(c, l);

        // Builder
        let (c, l) = Builder::test_initialized_data(&clickhouse, &libmdbx, None)
            .await
            .unwrap();
        assert_eq!(c, l);

        // AddressMeta
        let (c, l) = AddressMeta::test_initialized_data(&clickhouse, &libmdbx, None)
            .await
            .unwrap();
        assert_eq!(c, l);
    }
}
