use std::env;

use db_interfaces::clickhouse::{
    client::ClickhouseClient, config::ClickhouseConfig, dbms::NullDBMS,
};

#[allow(dead_code)]
pub(crate) fn get_clickhouse_env() -> ClickhouseClient<NullDBMS> {
    let user = env::var("CLICKHOUSE_USER").expect("CLICKHOUSE_USER not set");
    let password = env::var("CLICKHOUSE_PASS").expect("CLICKHOUSE_PASS not set");
    let url = format!(
        "{}:{}",
        env::var("CLICKHOUSE_URL").expect("CLICKHOUSE_URL not set"),
        env::var("CLICKHOUSE_PORT").expect("CLICKHOUSE_PORT not set")
    );

    ClickhouseConfig::new(user, password, url, true, None).build()
}
