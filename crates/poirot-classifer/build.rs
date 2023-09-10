use clickhouse::{Client, Row};
use ethers_core::types::H160;
use hyper_tls::HttpsConnector;
use serde::{Deserialize, Serialize};
use std::{
    env,
    fs::File,
    io::{BufWriter, Write},
    path::Path,
    str::FromStr,
};

const TOKEN_MAPPING: &str = "token_mappings.rs";
const TOKEN_QUERIES: &str = "SELECT toString(address), arrayMap(x -> toString(x),tokens) AS tokens FROM pools WHERE length(tokens) = ";

#[derive(Debug, Deserialize, Serialize, Row)]
pub struct DecodedTokens {
    address: String,
    tokens: Vec<String>,
}

fn main() {
    dotenv::dotenv().ok();
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();

    runtime.block_on(async {
        let client = build_db();

        let path = Path::new(&env::var("OUT_DIR").unwrap()).join(TOKEN_MAPPING);
        let mut file = BufWriter::new(File::create(&path).unwrap());

        for i in 2..4 {
            let res =
                query_db::<DecodedTokens>(&client, &(TOKEN_QUERIES.to_string() + &i.to_string()))
                    .await;

            build_token_map(i, res, &mut file)
        }
    });
}

async fn query_db<T: Row + for<'a> Deserialize<'a>>(db: &Client, query: &str) -> Vec<T> {
    db.query(query).fetch_all::<T>().await.unwrap()
}

fn to_string_vec(tokens: Vec<String>) -> String {
    let tokens = tokens.into_iter().map(|t| H160::from_str(&t).unwrap()).collect::<Vec<_>>();
    let mut res = "[".to_string();
    for token in tokens {
        res += "H160([";
        for byte in token.to_fixed_bytes() {
            res += &byte.to_string();
            res += ",";
        }
        let _ = res.pop();
        res += "]),";
    }
    let _ = res.pop();
    res += "]";

    res
}

fn build_token_map(amount: i32, rows: Vec<DecodedTokens>, file: &mut BufWriter<File>) {
    let mut phf_map = phf_codegen::Map::new();

    for row in rows {
        phf_map.entry(
            H160::from_str(&row.address).unwrap().to_fixed_bytes(),
            &to_string_vec(row.tokens),
        );
    }

    writeln!(
        file,
        "pub static ADDRESS_TO_TOKENS_{}_POOL: phf::Map<[u8; 20], [H160; {}]> = \n{};\n",
        amount,
        amount,
        phf_map.build()
    )
    .unwrap();
}

fn build_db() -> Client {
    // clickhouse path
    let clickhouse_path = format!(
        "{}:{}",
        &env::var("CLICKHOUSE_URL").expect("CLICKHOUSE_URL not found in .env"),
        &env::var("CLICKHOUSE_PORT").expect("CLICKHOUSE_PORT not found in .env")
    );

    // builds the https connector
    let https = HttpsConnector::new();
    let https_client = hyper::Client::builder().build::<_, hyper::Body>(https);

    // builds the clickhouse client

    Client::with_http_client(https_client)
        .with_url(clickhouse_path)
        .with_user(env::var("CLICKHOUSE_USER").expect("CLICKHOUSE_USER not found in .env"))
        .with_password(env::var("CLICKHOUSE_PASS").expect("CLICKHOUSE_PASS not found in .env"))
        .with_database(
            env::var("CLICKHOUSE_DATABASE").expect("CLICKHOUSE_DATABASE not found in .env"),
        )
}
