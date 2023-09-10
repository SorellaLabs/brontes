use std::{
    collections::HashMap,
    env,
    fs::{self, File},
    hash::Hash,
    io::{BufWriter, Write},
    path::Path
};

use ethers_core::types::Address;
use serde::{Deserialize, Serialize};
use strum::Display;

const TOKEN_MAPPING_FILE: &str = "token_mapping.rs";

#[derive(
    Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Serialize, Deserialize, Display,
)]
pub enum Blockchain {
    /// to represent an all query
    Optimism,
    Ethereum,
    Bsc,
    Gnosis,
    Polygon,
    Fantom,
    Klaytn,
    Arbitrum,
    Avalanche,
    Aurora
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenList {
    pub tokens: Vec<Token>
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
pub struct Token {
    pub chain_addresses: HashMap<Blockchain, Vec<Address>>,
    /// e.g USDC, USDT, ETH, BTC
    pub global_id:       String
}

impl Hash for Token {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.global_id.hash(state)
    }
}
fn main() {
    let tokens: TokenList = serde_json::from_str(
        &fs::read_to_string("../../ticker_address_mapping/assets.json").unwrap()
    )
    .unwrap();

    let path = Path::new(&env::var("OUT_DIR").unwrap()).join(TOKEN_MAPPING_FILE);
    let mut file = BufWriter::new(File::create(&path).unwrap());

    let mut phf_map = phf_codegen::Map::new();

    for mut token in tokens.tokens {
        let Some(eth_addrs) = token.chain_addresses.remove(&Blockchain::Ethereum) else { continue };
        for addr in eth_addrs {
            phf_map.entry(addr.0, &format!("\"{}\"", token.global_id));
        }
    }

    writeln!(
        &mut file,
        "pub static TOKEN_ADDRESS_TO_TICKER: phf::Map<[u8; 20], &'static str> = \n{};\n",
        phf_map.build()
    )
    .unwrap();
}
