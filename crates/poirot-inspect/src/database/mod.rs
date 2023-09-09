pub mod errors;
pub(crate) mod serialize;
pub mod types;



use sorella_db_clients::databases::clickhouse::ClickhouseClient;




const RELAYS_TABLE: &str = "relays";
const MEMPOOL_TABLE: &str = "chainbound_mempool";
const TARDIS_QUOTES_L2: &str = "tardis_l2";
const TARDIS_QUOTES_QUOTES: &str = "tardis_quotes";
const TARDIS_QUOTES_TRADES: &str = "tardis_trades";

#[derive(Default)]
pub struct InspectorDataClient {
    client: ClickhouseClient,
}



impl InspectorDataClient {}
