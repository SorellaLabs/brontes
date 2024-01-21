use std::{env, error::Error};

use brontes_database::libmdbx::{
    cursor::CompressedCursor,
    tables::{CompressedTable, IntoTableKey, Tables},
    Libmdbx,
};
use clap::Parser;
use itertools::Itertools;
use reth_db::mdbx::RO;

#[derive(Debug, Parser)]
pub struct DatabaseQuery {
    /// that table to be queried
    #[arg(long, short)]
    pub table: Tables,
    /// the key of the table being queried. if a range is wanted use the rust
    /// syntax of ..
    /// --key 80
    /// or --key 80..100
    #[arg(long, short)]
    pub key:   String,
}

impl DatabaseQuery {
    pub async fn execute(self) -> Result<(), Box<dyn Error>> {
        let brontes_db_endpoint = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");
        let db = Libmdbx::init_db(brontes_db_endpoint, None)?;

        let tx = db.ro_tx()?;

        macro_rules! match_table {
        ($table:expr, $fn:expr, $query:ident, $($tables:ident),+ = $args:expr) => {
            match $table {
                $(
                    Tables::$tables => {
                        println!(
                            "{:#?}",
                            $fn(
                                tx.$query::<brontes_database::libmdbx::tables::$tables>(
                                    brontes_database::libmdbx::tables::$tables::into_key($args)
                                    ).unwrap(),
                            ).unwrap()
                        )
                    }
                )+
            }
        };
        ($table:expr, $fn:expr, $query:ident, $($tables:ident),+) => {
            match $table {
                $(
                    Tables::$tables => {
                        println!(
                            "{:#?}",
                            $fn(
                                tx.$query::<brontes_database::libmdbx::tables::$tables>()?, self
                            )?
                        )
                    }
                )+
            }
        };
    }

        if self.key.contains("..") {
            match_table!(
                self.table,
                process_range_query,
                new_cursor,
                CexPrice,
                Metadata,
                DexPrice,
                MevBlocks,
                TokenDecimals,
                AddressToTokens,
                AddressToProtocol,
                PoolCreationBlocks,
                AddressToFactory,
                SubGraphs
            );
        } else {
            match_table!(
                self.table,
                process_single_query,
                get,
                CexPrice,
                Metadata,
                DexPrice,
                MevBlocks,
                TokenDecimals,
                AddressToTokens,
                AddressToProtocol,
                AddressToFactory,
                SubGraphs,
                PoolCreationBlocks = &self.key
            );
        }

        Ok(())
    }
}

fn process_range_query<T, E>(
    mut cursor: CompressedCursor<T, RO>,
    config: DatabaseQuery,
) -> Result<Vec<T::DecompressedValue>, Box<dyn Error>>
where
    T: CompressedTable,
    T: for<'a> IntoTableKey<&'a str, T::Key, E>,
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
{
    let range = config.key.split("..").collect_vec();
    let start = range[0];
    let end = range[1];

    let start = T::into_key(start);
    let end = T::into_key(end);

    let mut res = Vec::new();
    for entry in cursor.walk_range(start..end)? {
        if let Ok(entry) = entry {
            res.push(entry.1)
        }
    }

    Ok(res)
}

#[inline(always)]
fn process_single_query<T>(res: Option<T>) -> Result<T, Box<dyn Error>> {
    Ok(res.ok_or_else(|| reth_db::DatabaseError::Read(-1))?)
}
