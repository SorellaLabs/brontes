use std::{fs::File, sync::Arc};

use parquet::{file::writer::SerializedFileWriter, schema::parser::parse_message_type};
use serde::Deserialize;
use sorella_db_databases::{ClickhouseClient, Database, Row};

pub fn db_setup<D: Row + for<'a> Deserialize<'a> + Send + Sync>(
    query: &str,
    out_file: &str,
    schema_str: &str,
) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let database = ClickhouseClient::default();

    let data = rt.block_on(database.query_many::<D>(query, &())).unwrap();
    let schema = Arc::new(parse_message_type(schema_str).unwrap());

    let file = File::create(out_file).unwrap();
    let mut writer = SerializedFileWriter::new(file, schema, Default::default()).unwrap();

    writer.close().unwrap();
}

pub fn read_parquet<D: From<Row>>(file_path: &str) -> Vec<D> {
    let file = File::open(file_path)?;

    let reader = SerializedFileReader::new(file)?;
    let rows = reader.get_row_iter(None)?;

    rows.into_iter().map(Into::into).collect()
}
