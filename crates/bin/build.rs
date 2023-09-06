use clickhouse::{Client, Row};
use serde::{Deserialize, Serialize};
use std::env;

const PROTOCOL_ADDRESS_QUERY: &str = "
SELECT
    protocol,
    any(address) AS address
FROM pools
GROUP BY protocol
";

#[derive(Serialize, Deserialize, Row)]
struct ProtocolAbiAddress {
    protocol: String,
    address: String,
}

fn main() {
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();

    let clickhouse_client = build_db();
}

/// builds the clickhouse database client
fn build_db() -> Client {
    // clickhouse path
    let clickhouse_path = format!(
        "{}:{}",
        &env::var("CLICKHOUSE_URL").expect("CLICKHOUSE_URL not found in .env"),
        &env::var("CLICKHOUSE_PORT").expect("CLICKHOUSE_PORT not found in .env")
    );

    // builds the clickhouse client
    let http_client = hyper::Client::builder().pool_max_idle_per_host(10).build_http();
    let client = Client::with_http_client(http_client)
        .with_url(clickhouse_path)
        .with_user(env::var("CLICKHOUSE_USER").expect("CLICKHOUSE_USER not found in .env"))
        .with_password(env::var("CLICKHOUSE_PASS").expect("CLICKHOUSE_PASS not found in .env"))
        .with_database(
            env::var("CLICKHOUSE_DATABASE").expect("CLICKHOUSE_DATABASE not found in .env"),
        );
    client
}

/// queries the db
async fn query_db(db: Client, query: String) -> Vec<ProtocolAbiAddress> {
    db.query(&query).fetch_all::<ProtocolAbiAddress>().await.unwrap()
}
