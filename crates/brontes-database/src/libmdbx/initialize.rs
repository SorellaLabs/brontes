use std::{
    fmt::Debug,
    sync::{Arc, Mutex},
};

use brontes_types::{
    traits::TracingProvider,
    unordered_buffer_map::{BrontesStreamExt, UnorderedBufferMap},
};
use futures::{future::join_all, stream::iter, StreamExt};
use itertools::Itertools;
use reth_db::DatabaseError;
use serde::Deserialize;
use sorella_db_databases::{clickhouse::DbRow, Database};
use tracing::{error, info};

use super::{tables::Tables, types::LibmdbxData, Libmdbx};
use crate::{clickhouse::Clickhouse, libmdbx::types::CompressedTable};

const DEFAULT_START_BLOCK: u64 = 0;
const INNER_CHUNK_SIZE: usize = 10_000;

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
        block_range: Option<(u64, u64)>, // inclusive of start only
    ) -> eyre::Result<()> {
        join_all(
            tables
                .iter()
                .map(|table| table.initialize_table(&self, block_range)),
        )
        .await
        .into_iter()
        .collect::<eyre::Result<_>>()
    }

    pub(crate) async fn clickhouse_init_no_args<'db, T, D>(&'db self) -> eyre::Result<()>
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
        D: LibmdbxData<T> + DbRow + for<'de> Deserialize<'de> + Send + Sync + Debug + 'static,
    {
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
    ) -> eyre::Result<()>
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
        D: LibmdbxData<T> + DbRow + for<'de> Deserialize<'de> + Send + Sync + Debug + 'static,
    {
        self.libmdbx.clear_table::<T>()?;

        let block_range_chunks = if let Some((s, e)) = block_range {
            (s..e).chunks(T::INIT_CHUNK_SIZE.unwrap_or((e - s + 1) as usize))
        } else {
            let end_block = self.tracer.best_block_number()?;

            (DEFAULT_START_BLOCK..end_block).chunks(
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

        println!("NUM CHUNKS {}", pair_ranges.len());

        let num_chunks = Arc::new(Mutex::new(pair_ranges.len()));

        info!(target: "brontes::init", "{} -- Starting Initialization With {} Chunks", T::NAME, pair_ranges.len());
        iter(pair_ranges.into_iter().map(|(start, end)| {
            let num_chunks = num_chunks.clone();
            async move {
                iter(&(start..end).into_iter().chunks(INNER_CHUNK_SIZE)).map(|range| async {

                    let mut range = range.collect_vec();
                    let start = range.remove(0);
                    let end = range.pop().unwrap();
                    let clickhouse = self.clickhouse.clone();
                    let libmdbx = self.libmdbx.clone();

                    let data =
                        clickhouse
                        .inner()
                        .query_many::<D>(T::INIT_QUERY.expect("Should only be called on clickhouse tables"), &(start, end))
                        .await;

                    match data {
                        Ok(d) => libmdbx.write_table(&d)?,
                        Err(e) => {
                            info!(target: "brontes::init", "{} -- Error Writing -- {:?}", T::NAME,  e)
                        }
                    }
                    Ok::<(), DatabaseError>(())

                }).map(|e| {}).unordered_buffer_map(5, |item| {})

                .collect::<Vec<_>>().await;


            let num = {
                let mut n = num_chunks.lock().unwrap();
                *n -= 1;
                n.clone() + 1
            };

            info!(target: "brontes::init", "{} -- Finished Chunk {}", T::NAME, num);

            Ok::<(), DatabaseError>(())
        }})).buffer_unordered(5).collect::<Vec<_>>().await.into_iter()
        .collect::<Result<Vec<_>, _>>()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{env, path::Path, sync::Arc};

    use alloy_primitives::TxHash;
    use brontes_pricing::types::PoolState;
    use brontes_types::{
        classified_mev::{BundleHeader, MevBlock, Sandwich},
        structured_trace::TxTrace,
    };
    use reth_db::{cursor::DbCursorRO, transaction::DbTx, DatabaseError};
    use reth_interfaces::provider::ProviderResult;
    use reth_primitives::{BlockId, BlockNumber, BlockNumberOrTag, Bytes, Header, B256};
    use reth_rpc::eth::error::EthResult;
    use reth_rpc_types::{state::StateOverride, BlockOverrides, CallRequest, TransactionReceipt};
    use reth_tracing_ext::TracingClient;
    use serial_test::serial;
    use tokio::sync::mpsc::unbounded_channel;

    use super::LibmdbxInitializer;
    use crate::{clickhouse::Clickhouse, libmdbx::*};

    #[derive(Default)]
    struct NoopTP;

    #[async_trait::async_trait]
    impl TracingProvider for NoopTP {
        async fn eth_call(
            &self,
            request: CallRequest,
            block_number: Option<BlockId>,
            state_overrides: Option<StateOverride>,
            block_overrides: Option<Box<BlockOverrides>>,
        ) -> ProviderResult<Bytes> {
            Ok(Default::default())
        }

        async fn block_hash_for_id(&self, block_num: u64) -> ProviderResult<Option<B256>> {
            Ok(None)
        }

        #[cfg(not(feature = "local"))]
        fn best_block_number(&self) -> ProviderResult<u64> {
            Ok(0)
        }

        #[cfg(feature = "local")]
        async fn best_block_number(&self) -> ProviderResult<u64>;

        async fn replay_block_transactions(
            &self,
            block_id: BlockId,
        ) -> EthResult<Option<Vec<TxTrace>>> {
            Ok(None)
        }

        async fn block_receipts(
            &self,
            number: BlockNumberOrTag,
        ) -> ProviderResult<Option<Vec<TransactionReceipt>>> {
            Ok(None)
        }

        async fn header_by_number(&self, number: BlockNumber) -> ProviderResult<Option<Header>> {
            Ok(None)
        }

        async fn block_and_tx_index(&self, hash: TxHash) -> ProviderResult<(u64, usize)> {
            Ok((0, 0))
        }
    }

    fn init_db() -> eyre::Result<Libmdbx> {
        dotenv::dotenv().ok();
        let brontes_db_path = env::var("BRONTES_DB_PATH").expect(
            "No
 BRONTES_DB_PATH in .env",
        );
        Libmdbx::init_db(brontes_db_path, None)
    }

    /*
       fn init_trace_parser<'a>(
           handle: Handle,
           metrics_tx: UnboundedSender<PoirotMetricEvents>,
           libmdbx: &'a LibmdbxReadWriterr,
           max_tasks: u32,
       ) -> TraceParser<'a, Box<dyn TracingProvider>, LibmdbxReadWriterr> {
           let db_path = env::var("DB_PATH").expect("No DB_PATH in .env");

           #[cfg(feature = "local")]
           let tracer = {
               let db_endpoint = env::var("RETH_ENDPOINT").expect(
                   "No db
    Endpoint in .env",
               );
               let db_port = env::var("RETH_PORT").expect("No DB port.env");
               let url = format!("{db_endpoint}:{db_port}");
               Box::new(LocalProvider::new(url)) as Box<dyn TracingProvider>
           };
           #[cfg(not(feature = "local"))]
           let tracer = {
               let executor = TaskManager::new(handle.clone());
               let client =
                   TracingClient::new(Path::new(&db_path), max_tasks as u64, executor.executor());
               handle.spawn(executor);
               Box::new(client) as Box<dyn TracingProvider>
           };

           let call = Box::new(|_: &_, _: &_| true);

           TraceParser::new(libmdbx, call, Arc::new(tracer), Arc::new(metrics_tx))
       }  */

    async fn initialize_tables(tables: &[Tables]) -> eyre::Result<Arc<Libmdbx>> {
        let db = Arc::new(init_db()?);

        let clickhouse = Clickhouse::default();

        let trace_parser = NoopTP::default();

        let db_initializer =
            LibmdbxInitializer::new(db.clone(), Arc::new(clickhouse), trace_parser.into());
        db_initializer
            .initialize(tables, Some((15900000, 16000000)))
            .await?;

        Ok(db)
    }

    /*
       async fn initialize_tables(tables: &[Tables]) ->
    eyre::Result<Arc<Libmdbx>> {        let db = Arc::new(init_db()?);
           let clickhouse = Clickhouse::default();

           let db_path = env::var("DB_PATH")
               .map_err(|_| Box::new(std::env::VarError::NotPresent))
               .unwrap();
           let (manager, tracer) =
               TracingClient::new(Path::new(&db_path),
    tokio::runtime::Handle::current(), 10);        tokio::spawn(manager);

           let tracer = Arc::new(tracer);
           let db_initializer = LibmdbxInitializer::new(db.clone(),
    Arc::new(clickhouse), tracer);        db_initializer.
    initialize(tables, None).await?;

           Ok(db)
       }
    */
    async fn test_tokens_decimals_table(db: &Libmdbx, print: bool) -> eyre::Result<()> {
        let tx = db.ro_tx()?;
        assert_ne!(tx.entries::<TokenDecimals>()?, 0);

        let mut cursor = tx.cursor_read::<TokenDecimals>()?;
        if !print {
            cursor.first()?.ok_or(DatabaseError::Read(-1))?;
        } else {
            while let Some(vals) = cursor.next()? {
                println!("{:?}", vals);
            }
        }

        Ok(())
    }

    async fn test_address_to_tokens_table(db: &Libmdbx, print: bool) -> eyre::Result<()> {
        let tx = db.ro_tx()?;
        assert_ne!(tx.entries::<AddressToTokens>()?, 0);

        let mut cursor = tx.cursor_read::<AddressToTokens>()?;
        if !print {
            cursor.first()?.ok_or(DatabaseError::Read(-1))?;
        } else {
            while let Some(vals) = cursor.next()? {
                println!("{:?}", vals);
            }
        }
        Ok(())
    }

    async fn test_address_to_protocols_table(db: &Libmdbx, print: bool) -> eyre::Result<()> {
        let tx = db.ro_tx()?;
        assert_ne!(tx.entries::<AddressToProtocol>()?, 0);

        let mut cursor = tx.cursor_read::<AddressToProtocol>()?;
        if !print {
            cursor.first()?.ok_or(DatabaseError::Read(-1))?;
        } else {
            while let Some(vals) = cursor.next()? {
                println!("{:?}", vals);
            }
        }
        Ok(())
    }

    async fn test_cex_mapping_table(db: &Libmdbx, print: bool) -> eyre::Result<()> {
        let tx = db.ro_tx()?;
        assert_ne!(tx.entries::<CexPrice>()?, 0);

        let mut cursor = tx.cursor_read::<CexPrice>()?;
        if !print {
            cursor.first()?.ok_or(DatabaseError::Read(-1))?;
        } else {
            while let Some(vals) = cursor.next()? {
                println!("{:?}", vals);
            }
        }
        Ok(())
    }

    async fn test_metadata_table(db: &Libmdbx, print: bool) -> eyre::Result<()> {
        let tx = db.ro_tx()?;
        assert_ne!(tx.entries::<Metadata>()?, 0);

        let mut cursor = tx.cursor_read::<Metadata>()?;
        if !print {
            cursor.first()?.ok_or(DatabaseError::Read(-1))?;
        } else {
            while let Some(vals) = cursor.next()? {
                println!("{:?}", vals);
            }
        }
        Ok(())
    }
    /*
    async fn test_pool_state_table(db: &Libmdbx, print: bool) ->
    eyre::Result<()> {     let tx = LibmdbxTx::new_ro_tx(&db.0)?;
        assert_ne!(tx.entries::<PoolState>()?, 0);

        let mut cursor = tx.cursor_read::<PoolState>()?;
        if !print {
            cursor.first()?.ok_or(DatabaseError::Read(-1))?;
        } else {
            while let Some(vals) = cursor.next()? {
                println!("{:?}", vals);
            }
        }
        Ok(())
    }


        async fn test_dex_price_table(db: &Libmdbx, print: bool) ->
    eyre::Result<()> {         let tx = LibmdbxTx::new_ro_tx(&db.0)?;
            assert_ne!(tx.entries::<DexPrice>()?, 0);

            let mut cursor = tx.cursor_dup_read::<DexPrice>()?;

            if !print {
                cursor.first()?.ok_or(DatabaseError::Read(-1))?;
            } else {
                while let Some(vals) = cursor.next()? {
                    println!("{:?}\n", vals);
                }
            }

            println!("\n\n\n\n");

            cursor.first()?;
            let mut dup_walk = cursor.walk_dup(Some(10), None)?;
            if !print {
                let _ = dup_walk.next().ok_or(DatabaseError::Read(-1))?;
            } else {
                while let Some(vals) = dup_walk.next() {
                    println!("{:?}\n", vals);
                }
            }
            /*
            assert!(first_dup.is_some());
            println!("\n\n{:?}", first_dup);

            let next_dup = cursor.next_dup()?;
            assert!(next_dup.is_some());
            println!("\n\n{:?}", next_dup);
            */
            Ok(())
        }
    */
    async fn test_pool_creation_blocks_table(db: &Libmdbx, print: bool) -> eyre::Result<()> {
        let tx = db.ro_tx()?;
        assert_ne!(tx.entries::<PoolCreationBlocks>()?, 0);

        let mut cursor = tx.cursor_read::<PoolCreationBlocks>()?;
        if !print {
            cursor.first()?.ok_or(DatabaseError::Read(-1))?;
        } else {
            while let Some(vals) = cursor.next()? {
                println!("{:?}", vals);
            }
        }
        Ok(())
    }

    /*
    fn test_classified_mev_inserts(db: &Libmdbx) -> eyre::Result<()> {
        let block = MevBlock { ..Default::default() };
        let classified_mev = BundleHeader::default();
        let specific_mev = Sandwich::default();

        let _ = db.insert_classified_data(block, vec![(classified_mev, Box::new(specific_mev))]);

        Ok(())
    }


        async fn test_tx_traces_table(db: &Libmdbx, print: bool) ->
    eyre::Result<()> {         let tx = LibmdbxTx::new_ro_tx(&db.0)?;
            assert_ne!(tx.entries::<TxTracesDB>()?, 0);

            let mut cursor = tx.cursor_read::<TxTracesDB>()?;
            if !print {
                cursor.first()?.ok_or(DatabaseError::Read(-1))?;
            } else {
                while let Some(vals) = cursor.next()? {
                    println!("{:?}", vals);
                }
            }
            Ok(())
        }

    #[tokio::test(flavor = "multi_thread", worker_threads = 5)]
    #[serial]
    async fn test_inserts() {
        let db = init_db().unwrap();

        let q = test_classified_mev_inserts(&db);
        assert!(q.is_ok());
    }
    */
    #[tokio::test(flavor = "multi_thread", worker_threads = 20)]
    #[serial]
    async fn test_intialize_tables() {
        let db = initialize_tables(&[
            /*
            Tables::TokenDecimals,

            Tables::AddressToProtocol,
              */
            Tables::AddressToTokens,
            Tables::CexPrice,
            /*
            Tables::Metadata,
            Tables::PoolState,
            Tables::DexPrice,
            Tables::PoolCreationBlocks,
            Tables::TxTraces,
            */
        ])
        .await;
        assert!(db.is_ok());

        let db = db.unwrap();
        /*
        assert!(test_tokens_decimals_table(&db, false).await.is_ok());

        assert!(test_address_to_protocols_table(&db, false).await.is_ok());
        */
        assert!(test_address_to_tokens_table(&db, false).await.is_ok());
        //assert!(test_cex_mapping_table(&db, false).await.is_ok());
        /*
        assert!(test_metadata_table(&db, false).await.is_ok());
        assert!(test_pool_state_table(&db, false).await.is_ok());
        assert!(test_dex_price_table(&db, false).await.is_ok());
        assert!(test_pool_creation_blocks_table(&db, false).await.is_ok());
        assert!(test_tx_traces_table(&db, true).await.is_ok());
        */
    }
}
