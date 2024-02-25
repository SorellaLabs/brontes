#[cfg(not(feature = "local-clickhouse"))]
use std::env;
use std::fmt::Debug;

use ::clickhouse::DbRow;
use serde::Deserialize;

#[cfg(feature = "local-clickhouse")]
use crate::clickhouse::Clickhouse;
use crate::{
    clickhouse::ClickhouseHandle,
    libmdbx::{
        implementation::compressed_wrappers::utils::CompressedTableRow, types::LibmdbxData,
        LibmdbxReadWriter,
    },
    CompressedTable,
};
#[cfg(feature = "local-clickhouse")]
pub fn load_clickhouse() -> Clickhouse {
    Clickhouse::default()
}

#[cfg(not(feature = "local-clickhouse"))]
pub fn load_clickhouse() -> crate::clickhouse::ClickhouseHttpClient {
    let clickhouse_api = env::var("CLICKHOUSE_API").expect("No CLICKHOUSE_API in .env");
    let clickhouse_api_key = env::var("CLICKHOUSE_API_KEY");
    crate::clickhouse::ClickhouseHttpClient::new(clickhouse_api, clickhouse_api_key)
}

pub async fn clickhouse_data<T, D, CH: ClickhouseHandle>(
    clickhouse: &CH,
    block_range: Option<(u64, u64)>,
) -> eyre::Result<Vec<CompressedTableRow<T>>>
where
    T: CompressedTable,
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    D: LibmdbxData<T> + DbRow + for<'de> Deserialize<'de> + Send + Sync + Debug + 'static,
{
    let data = if let Some(rang) = block_range {
        clickhouse.query_many_range::<T, D>(rang.0, rang.1).await?
    } else {
        clickhouse.query_many::<T, D>().await?
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

pub async fn compare_clickhouse_libmdbx_data<T, D, CH: ClickhouseHandle>(
    clickhouse: &CH,
    libmdbx: &LibmdbxReadWriter,
    block_range: Option<(u64, u64)>,
) -> eyre::Result<()>
where
    T: CompressedTable,
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    D: LibmdbxData<T> + DbRow + for<'de> Deserialize<'de> + Send + Sync + Debug + 'static,
{
    let _clickhouse_data = clickhouse_data::<T, D, CH>(clickhouse, block_range).await?;

    let tx = libmdbx.0.ro_tx()?;
    let _libmdbx_data = tx
        .cursor_read::<T>()?
        .walk_range(..)?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(())
}
