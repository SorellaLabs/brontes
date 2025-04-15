use brontes_database::{
    libmdbx::{cursor::CompressedCursor, Libmdbx},
    CompressedTable, IntoTableKey, Tables,
};
use brontes_libmdbx::RO;
use brontes_types::init_thread_pools;
use clap::Parser;
use itertools::Itertools;
use reth_provider::errors::db::DatabaseErrorInfo;

#[derive(Debug, Parser)]
pub struct DatabaseQuery {
    /// Table to query
    #[arg(long, short)]
    pub table: Tables,
    /// Key for table query. Use Rust range syntax for ranges:
    /// --key 80 (single key)
    /// --key 80..100 (range)
    #[arg(long, short)]
    pub key:   String,
}

impl DatabaseQuery {
    pub async fn execute(self, brontes_db_path: String) -> eyre::Result<()> {
        init_thread_pools(10);
        let db = Libmdbx::init_db(brontes_db_path, None)?;

        db.view_db(|tx| {
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
                    CexTrades,
                    InitializedState,
                    BlockInfo,
                    DexPrice,
                    MevBlocks,
                    TokenDecimals,
                    AddressToProtocolInfo,
                    PoolCreationBlocks,
                    Builder,
                    AddressMeta,
                    SearcherEOAs,
                    SearcherContracts,
                    TxTraces
                );
            } else {
                match_table!(
                    self.table,
                    process_single_query,
                    get,
                    CexPrice,
                    CexTrades,
                    BlockInfo,
                    DexPrice,
                    MevBlocks,
                    TokenDecimals,
                    AddressToProtocolInfo,
                    Builder,
                    InitializedState,
                    AddressMeta,
                    SearcherEOAs,
                    SearcherContracts,
                    TxTraces,
                    PoolCreationBlocks = &self.key
                );
            }

            Ok(())
        })?;
        Ok(())
    }
}

fn process_range_query<T, E>(
    mut cursor: CompressedCursor<T, RO>,
    config: DatabaseQuery,
) -> eyre::Result<Vec<T::DecompressedValue>>
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
    for entry in cursor.walk_range(start..end)?.flatten() {
        res.push(entry.1);
    }

    Ok(res)
}

#[inline(always)]
fn process_single_query<T>(res: Option<T>) -> eyre::Result<T> {
    Ok(res.ok_or_else(|| reth_db::DatabaseError::Read(DatabaseErrorInfo::from(-1)))?)
}
