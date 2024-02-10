use std::fmt::Debug;

use serde::Deserialize;
use sorella_db_databases::{clickhouse::DbRow, Database};

use crate::{
    clickhouse::Clickhouse,
    libmdbx::{
        implementation::compressed_wrappers::utils::CompressedTableRow, types::LibmdbxData,
        LibmdbxReadWriter,
    },
    CompressedTable,
};

pub fn init_clickhouse() -> Clickhouse {
    dotenv::dotenv().ok();

    Clickhouse::default()
}

pub async fn clickhouse_data<T, D>(
    clickhouse: &Clickhouse,
    block_range: Option<(u64, u64)>,
) -> eyre::Result<Vec<CompressedTableRow<T>>>
where
    T: CompressedTable,
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    D: LibmdbxData<T> + DbRow + for<'de> Deserialize<'de> + Send + Sync + Debug + 'static,
{
    let data = if let Some(rang) = block_range {
        clickhouse
            .inner()
            .query_many::<D>(
                T::INIT_QUERY.expect("Should only be called on clickhouse tables"),
                &rang,
            )
            .await?
    } else {
        clickhouse
            .inner()
            .query_many::<D>(
                T::INIT_QUERY.expect("Should only be called on clickhouse tables"),
                &(),
            )
            .await?
    };

    let table_row = data
        .into_iter()
        .map(|val| {
            let key_val = val.into_key_val();
            CompressedTableRow(key_val.key, key_val.value)
        })
        .collect();

    Ok(table_row)
}

pub async fn compare_clickhouse_libmdbx_data<T, D>(
    clickhouse: &Clickhouse,
    libmdbx: &LibmdbxReadWriter,
    block_range: Option<(u64, u64)>,
) -> eyre::Result<()>
where
    T: CompressedTable,
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    D: LibmdbxData<T> + DbRow + for<'de> Deserialize<'de> + Send + Sync + Debug + 'static,
{
    let _clickhouse_data = clickhouse_data::<T, D>(clickhouse, block_range).await?;

    let tx = libmdbx.0.ro_tx()?;
    let _libmdbx_data = tx
        .cursor_read::<T>()?
        .walk_range(..)?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(())
}
