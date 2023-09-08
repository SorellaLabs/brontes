use serde::Deserialize;
use std::env;

use clickhouse::{Client, Row};
use hyper_tls::HttpsConnector;

pub struct InspectorDataClient {
    client: Client,
}
impl InspectorDataClient {
    pub fn new() -> Self {
        let clickhouse_path = format!(
            "{}:{}",
            &env::var("CLICKHOUSE_URL").expect("CLICKHOUSE_URL not found in .env"),
            &env::var("CLICKHOUSE_PORT").expect("CLICKHOUSE_PORT not found in .env")
        );

        // builds the https connector
        let https = HttpsConnector::new();
        let https_client = hyper::Client::builder().build::<_, hyper::Body>(https);

        // builds the clickhouse client
        let client = Client::with_http_client(https_client)
            .with_url(clickhouse_path)
            .with_user(env::var("CLICKHOUSE_USER").expect("CLICKHOUSE_USER not found in .env"))
            .with_password(env::var("CLICKHOUSE_PASS").expect("CLICKHOUSE_PASS not found in .env"))
            .with_database(
                env::var("CLICKHOUSE_DATABASE").expect("CLICKHOUSE_DATABASE not found in .env"),
            );
        Self { client }
    }

    async fn query_db<T: Row + for<'a> Deserialize<'a>>(&self, query: &str) -> Vec<T> {
        self.client.query(query).fetch_all::<T>().await.unwrap()
    }
}
