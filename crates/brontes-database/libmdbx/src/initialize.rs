use std::sync::Arc;

use brontes_database::clickhouse::Clickhouse;
use futures::future::join_all;

use super::{tables::Tables, Libmdbx};

pub struct LibmdbxInitializer<'db> {
    libmdbx:    &'db Libmdbx,
    clickhouse: &'db Clickhouse,
}

impl<'db> LibmdbxInitializer<'db> {
    pub fn new(libmdbx: &'db Libmdbx, clickhouse: &'db Clickhouse) -> Self {
        Self { libmdbx, clickhouse }
    }

    pub async fn initialize(
        &self,
        tables: &[Tables],
        block_range: Option<(u64, u64)>, // inclusive of start only
    ) -> eyre::Result<()> {
        let clickhouse = Arc::new(self.clickhouse);
        join_all(
            tables
                .iter()
                .map(|table| table.initialize_table(self.libmdbx, clickhouse.clone(), block_range)),
        )
        .await
        .into_iter()
        .collect::<eyre::Result<_>>()
    }
}

#[cfg(test)]
mod tests {
    use std::env;

    use brontes_database::clickhouse::Clickhouse;
    use brontes_pricing::{
        types::PoolStateSnapShot, uniswap_v2::UniswapV2Pool, uniswap_v3::UniswapV3Pool,
    };
    use reth_db::{
        cursor::{self, DbCursorRO, DbDupCursorRO},
        transaction::DbTx,
        DatabaseError,
    };
    use serial_test::serial;
    use sorella_db_databases::{clickhouse, ClickhouseClient, Row};

    use crate::{
        implementation::tx::LibmdbxTx,
        initialize::LibmdbxInitializer,
        tables::{
            AddressToProtocol, AddressToTokens, CexPrice, DexPrice, Metadata, PoolState, Tables,
            TokenDecimals,
        },
        types::{
            address_to_protocol::{AddressToProtocolData, StaticBindingsDb},
            pool_state::{PoolStateData, PoolStateType},
        },
        Libmdbx,
    };

    fn init_db() -> eyre::Result<Libmdbx> {
        dotenv::dotenv().ok();
        let brontes_db_path = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");
        Libmdbx::init_db(brontes_db_path, None)
    }

    async fn initialize_tables(tables: &[Tables]) -> eyre::Result<Libmdbx> {
        let db = init_db()?;
        let clickhouse = Clickhouse::default();

        let db_initializer = LibmdbxInitializer::new(&db, &clickhouse);
        db_initializer.initialize(tables, None).await?;

        Ok(db)
    }

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

    #[tokio::test]
    #[serial]
    async fn test_intialize_tables() {
        let db = initialize_tables(&[
            Tables::TokenDecimals,
            Tables::AddressToTokens,
            Tables::AddressToProtocol,
            Tables::CexPrice,
            //Tables::Metadata,
            //Tables::PoolState,
            //Tables::DexPrice,
        ])
        .await;
        assert!(db.is_ok());

        //let db = db.unwrap();
        //assert!(test_tokens_decimals_table(&db, false).await.is_ok());
        //assert!(test_address_to_tokens_table(&db, false).await.is_ok());
        //assert!(test_address_to_protocols_table(&db, false).await.is_ok());
        //assert!(test_cex_mapping_table(&db, false).await.is_ok());
        //assert!(test_metadata_table(&db, false).await.is_ok());
        //assert!(test_pool_state_table(&db, false).await.is_ok());
        //assert!(test_dex_price_table(&db, false).await.is_ok());
    }
}
