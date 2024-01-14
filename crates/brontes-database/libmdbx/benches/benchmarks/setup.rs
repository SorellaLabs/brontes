use std::{env, fs::File, sync::Arc};

use arrow::{datatypes::Schema, record_batch::RecordBatch};
use brontes_database_libmdbx::Libmdbx;
use parquet::arrow::{arrow_reader::ParquetRecordBatchReaderBuilder, ArrowWriter};
use serde::Deserialize;
use sorella_db_databases::{ClickhouseClient, Database, Row};

pub trait ToRecordBatch: Sized {
    fn into_record_batch(rows: Vec<Self>) -> RecordBatch;
}

pub fn parquet_setup<D>(query: &str, out_file: &str, schema: Schema)
where
    D: Row + for<'a> Deserialize<'a> + Send + Sync + ToRecordBatch,
{
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let database = ClickhouseClient::default();

    println!("QUERYING DATA FROM CLICKHOUSE");

    let data = rt.block_on(database.query_many::<D>(query, &())).unwrap();

    let file = File::create(out_file).unwrap();
    let mut writer = ArrowWriter::try_new(file, Arc::new(schema.clone()), None).unwrap();

    println!("WRITING DATA TO PARQUET");

    writer.write(&D::into_record_batch(data)).unwrap();

    writer.close().unwrap();
}

pub fn read_parquet<D: From<RecordBatch>>(file_path: &str) -> Vec<D> {
    let file = File::open(file_path).unwrap();

    let builder = ParquetRecordBatchReaderBuilder::try_new(file).unwrap();
    let mut reader = builder.build().unwrap();

    println!("READING DATA FROM PARQUET");

    let mut rows = Vec::new();

    while let Some(row) = reader.next() {
        rows.push(row.map(|r| r.into()))
    }

    println!("READ DATA FROM PARQUET");

    rows.into_iter().collect::<Result<Vec<_>, _>>().unwrap()
}

pub fn init_db() -> Libmdbx {
    dotenv::dotenv().ok();
    let brontes_bench_db_path =
        env::var("BRONTES_LIBMDBX_BENCHES_PATH").expect("No BRONTES_LIBMDBX_BENCHES_PATH in .env");
    Libmdbx::init_db(&format!("{}{}", brontes_bench_db_path, "bench_db"), None).unwrap()
}
