use std::sync::Arc;

use brontes_database::clickhouse::Clickhouse;
use futures::future::join_all;
use reth_tracing_ext::TracingClient;

use super::{tables::Tables, Libmdbx};

pub struct LibmdbxInitializer {
    libmdbx:    Arc<Libmdbx>,
    clickhouse: Arc<Clickhouse>,
    //tracer:     Arc<TracingClient>,
}

impl LibmdbxInitializer {
    pub fn new(
        libmdbx: Arc<Libmdbx>,
        clickhouse: Arc<Clickhouse>,
        //tracer: Arc<TracingClient>,
    ) -> Self {
        Self { libmdbx, clickhouse } //, tracer }
    }

    pub async fn initialize(
        &self,
        tables: &[Tables],
        block_range: Option<(u64, u64)>, // inclusive of start only
    ) -> eyre::Result<()> {
        join_all(tables.iter().map(|table| {
            table.initialize_table(
                self.libmdbx.clone(),
                //self.tracer.clone(),
                self.clickhouse.clone(),
                block_range,
            )
        }))
        .await
        .into_iter()
        .collect::<eyre::Result<_>>()
    }
}

#[cfg(test)]
mod tests {
    use std::{env, path::Path, sync::Arc};

    use brontes_database::clickhouse::Clickhouse;
    use brontes_types::classified_mev::{ClassifiedMev, Sandwich};
    use reth_db::{cursor::DbCursorRO, transaction::DbTx, DatabaseError};
    use reth_tracing_ext::TracingClient;
    use serial_test::serial;

    use crate::{
        implementation::tx::LibmdbxTx,
        initialize::LibmdbxInitializer,
        tables::{
            AddressToProtocol, AddressToTokens, CexPrice, Metadata, MevBlocks, PoolCreationBlocks,
            PoolState, Tables, TokenDecimals, TxTracesDB,
        },
        Libmdbx, MevBlock,
    };

    fn init_db() -> eyre::Result<Libmdbx> {
        dotenv::dotenv().ok();
        let brontes_db_path = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");
        Libmdbx::init_db(brontes_db_path, None)
    }

    async fn initialize_tables(tables: &[Tables]) -> eyre::Result<Arc<Libmdbx>> {
        let db = Arc::new(init_db()?);
        let clickhouse = Clickhouse::default();

        let db_initializer = LibmdbxInitializer::new(db.clone(), Arc::new(clickhouse));
        db_initializer.initialize(tables, None).await?;

        Ok(db)
    }

    /*
       async fn initialize_tables(tables: &[Tables]) -> eyre::Result<Arc<Libmdbx>> {
           let db = Arc::new(init_db()?);
           let clickhouse = Clickhouse::default();

           let db_path = env::var("DB_PATH")
               .map_err(|_| Box::new(std::env::VarError::NotPresent))
               .unwrap();
           let (manager, tracer) =
               TracingClient::new(Path::new(&db_path), tokio::runtime::Handle::current(), 10);
           tokio::spawn(manager);

           let tracer = Arc::new(tracer);
           let db_initializer = LibmdbxInitializer::new(db.clone(), Arc::new(clickhouse), tracer);
           db_initializer.initialize(tables, None).await?;

           Ok(db)
       }
    */
    async fn test_tokens_decimals_table(db: &Libmdbx, print: bool) -> eyre::Result<()> {
        let tx = LibmdbxTx::new_ro_tx(&db.0)?;
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
        let tx = LibmdbxTx::new_ro_tx(&db.0)?;
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
        let tx = LibmdbxTx::new_ro_tx(&db.0)?;
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
        let tx = LibmdbxTx::new_ro_tx(&db.0)?;
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
        let tx = LibmdbxTx::new_ro_tx(&db.0)?;
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

    async fn test_pool_state_table(db: &Libmdbx, print: bool) -> eyre::Result<()> {
        let tx = LibmdbxTx::new_ro_tx(&db.0)?;
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

    /*
        async fn test_dex_price_table(db: &Libmdbx, print: bool) -> eyre::Result<()> {
            let tx = LibmdbxTx::new_ro_tx(&db.0)?;
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
        let tx = LibmdbxTx::new_ro_tx(&db.0)?;
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

    fn test_classified_mev_inserts(db: &Libmdbx) -> eyre::Result<()> {
        let block = MevBlock { ..Default::default() };
        let classified_mev = ClassifiedMev::default();
        let specific_mev = Sandwich::default();

        let _ = db.insert_classified_data(block, vec![(classified_mev, Box::new(specific_mev))]);

        Ok(())
    }

    async fn test_tx_traces_table(db: &Libmdbx, print: bool) -> eyre::Result<()> {
        let tx = LibmdbxTx::new_ro_tx(&db.0)?;
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 20)]
    #[serial]
    async fn test_intialize_tables() {
        let db = initialize_tables(&[
            //Tables::TokenDecimals,
            //Tables::AddressToTokens,
            //Tables::AddressToProtocol,
            //Tables::CexPrice,
            Tables::Metadata,
            //Tables::PoolState,
            //Tables::DexPrice,
            //Tables::PoolCreationBlocks,
            // Tables::TxTraces,
        ])
        .await;
        assert!(db.is_ok());

        let db = db.unwrap();
        //assert!(test_tokens_decimals_table(&db, false).await.is_ok());
        //assert!(test_address_to_tokens_table(&db, false).await.is_ok());
        //assert!(test_address_to_protocols_table(&db, false).await.is_ok());
        //assert!(test_cex_mapping_table(&db, false).await.is_ok());
        assert!(test_metadata_table(&db, false).await.is_ok());
        //assert!(test_pool_state_table(&db, false).await.is_ok());
        //assert!(test_dex_price_table(&db, false).await.is_ok());
        //assert!(test_pool_creation_blocks_table(&db, false).await.is_ok());
        // assert!(test_tx_traces_table(&db, true).await.is_ok());
    }
}
