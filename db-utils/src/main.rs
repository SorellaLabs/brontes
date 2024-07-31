use clickhouse::Row;
use db_interfaces::{
    clickhouse::{client::ClickhouseClient, dbms::NullDBMS},
    Database,
};
use serde::{Deserialize, Serialize};

const DELETE_QUERY: &str = "DELETE FROM TABLE WHERE tx_hash = ?";
const TABLES: [&str; 8] = [
    "mev.bundle_header",
    "mev.cex_dex",
    "mev.jit",
    "mev.jit_sandwich",
    "mev.sandwich",
    "mev.cex_dex_quotes",
    "mev.liquidations",
    "mev.atomic_arbs",
];

#[tokio::main]
async fn main() -> eyre::Result<()> {
    dotenv::dotenv().ok();

    let tx_hashes = vec!["0x"];

    let url = format!(
        "{}:{}",
        std::env::var("CLICKHOUSE_URL").expect("CLICKHOUSE_URL not found in .env"),
        std::env::var("CLICKHOUSE_PORT").expect("CLICKHOUSE_PORT not found in .env")
    );
    let user = std::env::var("CLICKHOUSE_USER").expect("CLICKHOUSE_USER not found in .env");
    let pass = std::env::var("CLICKHOUSE_PASS").expect("CLICKHOUSE_PASS not found in .env");

    let clickhouse: ClickhouseClient<NullDBMS> =
        db_interfaces::clickhouse::config::ClickhouseConfig::new(user, pass, url, true, None)
            .build();

    for tx in tx_hashes {
        futures::future::try_join_all(TABLES.iter().map(|table| {
            let query = DELETE_QUERY.replace("TABLE", table);
            clickhouse.execute_remote(query, &(tx))
        }))
        .await?;
    }

    Ok(())
}
