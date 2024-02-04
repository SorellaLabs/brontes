use std::{
    env, fs,
    fs::{self, File},
    io,
    io::{BufRead, Write},
    path,
    path::Path,
    str::FromStr,
};

use brontes_database::libmdbx::{tables::AddressToProtocolData, LibmdbxReadWriter, LibmdbxWriter};
use serde::Deserialize;
use toml::Table;

/// sql file directory
const CLICKHOUSE_FILE_DIRECTORY: &str = "./src/clickhouse/queries/";

/// sql file directory
const LIBMDBX_SQL_FILE_DIRECTORY: &str = "./src/libmdbx/tables/queries/";
const CONFIG_FILE_NAME: &str = "manual_inserts.toml";

fn main() {
    write_clickhouse_sql();
    write_libmdbx_sql();
    insert_manually_defined_entries()
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub protocols: Table,
}

#[derive(Debug, Deserialize)]
pub struct ProtocolTable {
    init_block: u64,
    token_info: Vec<TokenInfoWIthAddress>,
}

fn insert_manually_defined_entries() {
    let brontes_db_endpoint = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");

    let libmdbx =
        LibmdbxReadWriter::init_db(brontes_db_endpoint, None).expect("failed to init libmdbx");

    let mut workspace_dir = workspace_dir();
    workspace_dir.push(CONFIG_FILE_NAME);

    let config: Config =
        toml::from_str(&std::fs::read_to_string(workspace_dir).expect("no config file"))
            .expect("failed to parse toml");

    let mut entries = Vec::new();
    for (protocol, table_entries) in &config.protocols {
        let protocol = Protocol::from_str(protocol).unwrap();

        for (address, table_entry) in table_entries.as_table().unwrap() {
            let address: Address = address.parse().unwrap();

            let entry: ProtocolTable = toml::from_str(&table_entry.to_string()).unwrap();

            for t_info in &entry.token_info {
                libmdbx
                    .write_token_info(t_info.address, t_info.decimals, t_info.symbol)
                    .unwrap();
            }

            if entry.token_info.len() < 2 {
                panic!("Config entry missing token info");
            }

            let token_addrs = [entry.token_info[0].address, entry.token_info[1].address];
            libmdbx
                .insert_pool(entry.init_block, address, token_addrs, protocol)
                .unwrap()
        }
    }
}

/// writes the sql file as a string to ./src/const_sql.rs
/// '?' are parameters that need to be bound to
fn write_clickhouse_sql() {
    let dest_path = Path::new("./src/clickhouse/const_sql.rs");
    let mut f = File::create(dest_path).unwrap();
    writeln!(f, "pub use clickhouse_mod::*;\n#[rustfmt::skip]\nmod clickhouse_mod {{").unwrap();

    for entry in fs::read_dir(CLICKHOUSE_FILE_DIRECTORY).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

        if path.extension().unwrap() == "sql" {
            let sql_string = read_sql(path.to_str().unwrap());

            let const_name = path.file_stem().unwrap().to_str().unwrap().to_uppercase();
            writeln!(
                f,
                "#[allow(dead_code)]\npub const {}: &str = r#\"{}\"#;\n",
                const_name, sql_string
            )
            .unwrap();
        }
    }
    writeln!(f, "}}").unwrap();
}

fn write_libmdbx_sql() {
    let dest_path = Path::new("./src/libmdbx/tables/const_sql.rs");
    let mut f = File::create(dest_path).unwrap();
    writeln!(f, "pub use libmdbx_mod::*;\n#[rustfmt::skip]\nmod libmdbx_mod{{").unwrap();

    for entry in fs::read_dir(LIBMDBX_SQL_FILE_DIRECTORY).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

        if path.extension().unwrap() == "sql" {
            let sql_string = read_sql(path.to_str().unwrap());

            let const_name = path.file_stem().unwrap().to_str().unwrap();
            writeln!(
                f,
                "#[allow(dead_code)]\n#[allow(non_upper_case_globals)]\npub const {}: &str = \
                 r#\"{}\"#;\n",
                const_name, sql_string
            )
            .unwrap();
        }
    }

    writeln!(f, "}}").unwrap();
}

// Reads an SQL file into a string
fn read_sql(s: &str) -> String {
    fs::read_to_string(s).unwrap()
}

fn workspace_dir() -> path::PathBuf {
    let output = std::process::Command::new(env!("CARGO"))
        .arg("locate-project")
        .arg("--workspace")
        .arg("--message-format=plain")
        .output()
        .unwrap()
        .stdout;
    let cargo_path = path::Path::new(std::str::from_utf8(&output).unwrap().trim());
    cargo_path.parent().unwrap().to_path_buf()
}
