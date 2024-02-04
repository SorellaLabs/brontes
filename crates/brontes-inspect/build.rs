use std::{env, path};

use alloy_primitives::Address;
use brontes_database::libmdbx::{LibmdbxReadWriter, LibmdbxWriter};
use brontes_types::{db::token_info::TokenInfoWithAddress, Protocol};
use serde::Deserialize;
use toml::Table;

const CONFIG_FILE_NAME: &str = "manual_inserts.toml";

fn main() {
    insert_manually_defined_entries()
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub protocol_name: Protocol,
    pub pools:         Vec<ProtocolTable>,
}

#[derive(Debug, Deserialize)]
pub struct ProtocolTable {
    init_block: u64,
    token_info: Vec<TokenInfoWithAddress>,
}

fn insert_manually_defined_entries() {
    // don't run on local
    let Ok(brontes_db_endpoint) = env::var("BRONTES_DB_PATH") else { return };

    let Ok(libmdbx) = LibmdbxReadWriter::init_db(brontes_db_endpoint, None) else { return };

    let mut workspace_dir = workspace_dir();
    workspace_dir.push(CONFIG_FILE_NAME);

    let config: Table =
        toml::from_str(&std::fs::read_to_string(workspace_dir).expect("no config file"))
            .expect("failed to parse toml");

    for (protocol, inner) in config {
        let protocol: Protocol = protocol.parse().unwrap();
        for (address, table) in inner.as_table().unwrap() {
            let token_addr: Address = address.parse().unwrap();

            let table: ProtocolTable = toml::from_str(&table.to_string()).unwrap();

            for t_info in &table.token_info {
                libmdbx
                    .write_token_info(t_info.address, t_info.decimals, t_info.symbol.clone())
                    .unwrap();
            }

            if table.token_info.len() < 2 {
                panic!("Config entry missing token info");
            }

            let token_addrs = [table.token_info[0].address, table.token_info[1].address];
            libmdbx
                .insert_pool(table.init_block, token_addr, token_addrs, protocol)
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
