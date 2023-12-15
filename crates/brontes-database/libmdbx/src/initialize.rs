use brontes_database::clickhouse::Clickhouse;
use futures::future::join_all;

use super::{tables::Tables, Libmbdx};

pub struct LibmbdxInitializer<'db> {
    libmdbx:    &'db Libmbdx,
    clickhouse: &'db Clickhouse,
}

impl<'db> LibmbdxInitializer<'db> {
    pub fn new(libmdbx: &'db Libmbdx, clickhouse: &'db Clickhouse) -> Self {
        Self { libmdbx, clickhouse }
    }

    pub async fn initialize(&self, tables: &[Tables]) -> eyre::Result<()> {
        join_all(
            tables
                .iter()
                .map(|table| table.initialize_table(self.libmdbx, self.clickhouse)),
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
    use reth_db::{cursor::DbCursorRO, transaction::DbTx, DatabaseError};
    use serial_test::serial;

    use crate::libmdbx::{
        implementation::tx::LibmbdxTx,
        initialize::LibmbdxInitializer,
        tables::{AddressToTokens, Tables, TokenDecimals},
        Libmbdx,
    };

    fn init_db() -> eyre::Result<Libmbdx> {
        dotenv::dotenv().ok();
        let brontes_db_path = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");
        Libmbdx::init_db(brontes_db_path, None)
    }

    async fn initialize_tables() -> eyre::Result<Libmbdx> {
        let db = init_db()?;
        let clickhouse = Clickhouse::default();

        let db_initializer = LibmbdxInitializer::new(&db, &clickhouse);

        db_initializer.initialize(&Tables::ALL).await?;

        Ok(db)
    }

    async fn test_tokens_decimals_table(db: &Libmbdx, print: bool) -> eyre::Result<()> {
        let tx = LibmbdxTx::new_ro_tx(&db.0)?;
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

    async fn test_address_to_tokens_table(db: &Libmbdx, print: bool) -> eyre::Result<()> {
        let tx = LibmbdxTx::new_ro_tx(&db.0)?;
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

    #[tokio::test]
    #[serial]
    async fn test_intialize_tables() {
        let db = initialize_tables().await;
        assert!(db.is_ok());

        let db = db.unwrap();
        assert!(test_tokens_decimals_table(&db, false).await.is_ok());
        assert!(test_address_to_tokens_table(&db, true).await.is_ok());
    }
}
