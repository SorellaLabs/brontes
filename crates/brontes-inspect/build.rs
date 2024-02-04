use core::panic;
use std::{env, path};

use alloy_primitives::Address;
use brontes_database::libmdbx::{LibmdbxReadWriter, LibmdbxWriter};
use brontes_types::Protocol;
use serde::Deserialize;
use toml::Table;

const CONFIG_FILE_NAME: &str = "manual_inserts.toml";

fn main() {
    insert_manually_defined_entries()
}

#[derive(Debug, Deserialize)]
pub struct TokenInfoWithAddressToml {
    pub symbol:   String,
    pub decimals: u8,
    pub address:  Address,
}
fn insert_manually_defined_entries() {
    // don't run on local
    dotenv::dotenv().unwrap();
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
            let init_block = table.get("init_block").unwrap().as_integer().unwrap() as u64;

            let table: Vec<TokenInfoWithAddressToml> =
                 table.get("token_info").unwrap().clone().try_into().unwrap();


            for t_info in &table {
                libmdbx
                    .write_token_info(t_info.address, t_info.decimals, t_info.symbol.clone())
                    .unwrap();
            }

            if table.len() < 2 {
                panic!("Config entry missing token info");
            }

            let token_addrs = [table[0].address, table[1].address];
            libmdbx
                .insert_pool(init_block, token_addr, token_addrs, protocol)
                .unwrap();
            // not reaching here
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
