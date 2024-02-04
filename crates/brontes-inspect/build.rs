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

const CONFIG_FILE_NAME: &str = "manual_inserts.toml";

fn main() {
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
