use std::{env, fmt::Debug};

#[cfg(feature = "local")]
use brontes_core::local_provider::LocalProvider;
#[cfg(not(feature = "local"))]
use reth_tasks::TaskManager;
#[cfg(not(feature = "local"))]
use reth_tracing_ext::TracingClient;
use serde::Deserialize;
use sorella_db_databases::{clickhouse::DbRow, Database};
#[cfg(not(feature = "local"))]
use tokio::runtime::Handle;

use super::{
    implementation::compressed_wrappers::utils::CompressedTableRow, types::LibmdbxData,
    LibmdbxReadWriter,
};
use crate::{clickhouse::Clickhouse, CompressedTable};

pub fn init_libmdbx() -> eyre::Result<&'static LibmdbxReadWriter> {
    dotenv::dotenv().ok();
    let brontes_test_db_path =
        env::var("BRONTES_TEST_DB_PATH").expect("No BRONTES_TEST_DB_PATH in .env");
    Ok(Box::leak(Box::new(LibmdbxReadWriter::init_db(brontes_test_db_path, None)?)))
}

#[cfg(not(feature = "local"))]
pub fn init_tracing(handle: Handle) -> eyre::Result<TracingClient> {
    dotenv::dotenv().ok();

    let task_manager = TaskManager::new(handle);
    let reth_db_path = env::var("DB_PATH").expect("No DB_PATH in .env");

    Ok(TracingClient::new(&std::path::Path::new(&reth_db_path), 10, task_manager.executor()))
}

#[cfg(feature = "local")]
pub fn init_tracing() -> eyre::Result<LocalProvider> {
    dotenv::dotenv().ok();

    let reth_http_endpoint = env::var("RETH_ENDPOINT").expect("No RETH_ENDPOINT in .env");

    Ok(LocalProvider::new(reth_http_endpoint))
}

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
) -> eyre::Result<(usize, usize)>
where
    T: CompressedTable,
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
    D: LibmdbxData<T> + DbRow + for<'de> Deserialize<'de> + Send + Sync + Debug + 'static,
{
    let clickhouse_data = clickhouse_data::<T, D>(clickhouse, block_range).await?;

    let tx = libmdbx.0.ro_tx()?;
    let libmdbx_data = tx
        .cursor_read::<T>()?
        .walk_range(..)?
        .collect::<Result<Vec<_>, _>>()?;

    Ok((clickhouse_data.len(), libmdbx_data.len()))
}
