use std::env;

use ahash::HashSetExt;
use alloy_primitives::Address;
use brontes_core::LibmdbxReader;
use brontes_database::clickhouse::cex_config::CexDownloadConfig;
use brontes_types::{
    db::cex::{CexExchange, CexTrades},
    init_threadpools,
    pair::Pair,
    FastHashMap, FastHashSet,
};
use clap::Parser;
use clickhouse::Row;
use db_interfaces::{
    clickhouse::{
        client::ClickhouseClient,
        config::ClickhouseConfig,
        dbms::{ClickhouseDBMS, NullDBMS},
    },
    Database,
};
use eyre::Ok;
use serde::{Deserialize, Serialize};

use crate::{
    cli::{load_libmdbx, static_object},
    runner::CliContext,
};

#[derive(Debug, Parser)]
pub struct CexDB {
    /// The block number
    #[arg(long, short)]
    pub block_number: u64,
    /// The first token in the pair
    #[arg(long)]
    pub token_0:      String,
    #[arg(long)]
    /// The second token in the pair
    pub token_1:      String,
    #[arg(long, short)]
    pub volume:       Option<f64>,
}

impl CexDB {
    pub async fn execute(self, brontes_db_endpoint: String, ctx: CliContext) -> eyre::Result<()> {
        init_threadpools(10);

        let task_executor = ctx.task_executor;

        let cex_config = CexDownloadConfig::default();

        let libmdbx = static_object(load_libmdbx(&task_executor, brontes_db_endpoint)?);

        let metadata = libmdbx.get_metadata(self.block_number)?;

        let clickhouse: ClickhouseClient<NullDBMS> = get_clickhouse_env();

        let token0: Address = self.token_0.parse()?;
        let token1: Address = self.token_1.parse()?;

        let pair = Pair(token0, token1);

        let block_timestamp = metadata.microseconds_block_timestamp();

        let intermediary_addresses = calculate_intermediary_addresses(
            &metadata.cex_trades.unwrap().lock().0,
            &cex_config.exchanges_to_use,
            &pair,
        );

        let start_timestamp = block_timestamp - cex_config.time_window.0 as u64 * 1000000;

        let end_timestamp = block_timestamp + cex_config.time_window.1 as u64 * 1000000;

        process_pair(&clickhouse, pair, start_timestamp, end_timestamp).await?;
        process_intermediaries(
            &clickhouse,
            pair,
            intermediary_addresses,
            start_timestamp,
            end_timestamp,
        )
        .await?;

        Ok(())
    }
}

async fn process_intermediaries<D: ClickhouseDBMS>(
    clickhouse: &ClickhouseClient<D>,
    pair: Pair,
    intermediaries: FastHashSet<Address>,
    start_timestamp: u64,
    end_timestamp: u64,
) -> Result<(), eyre::Report> {
    for intermediary in intermediaries {
        let intermediary_pair_1 = Pair(pair.0, intermediary);
        let intermediary_pair_2 = Pair(intermediary, pair.1);

        println!("Processing intermediary: {:?}", intermediary);

        // Query for the first intermediary pair
        let pair_info_1 = query_trading_pair_info(clickhouse, intermediary_pair_1).await?;

        let stats_1 = query_trade_stats(
            clickhouse,
            &pair_info_1.trading_pair,
            start_timestamp,
            end_timestamp,
        )
        .await?;
        print_trade_stats(&stats_1);

        // Query for the second intermediary pair
        let pair_info_2 = query_trading_pair_info(clickhouse, intermediary_pair_2).await?;

        let stats_2 = query_trade_stats(
            clickhouse,
            &pair_info_2.trading_pair,
            start_timestamp,
            end_timestamp,
        )
        .await?;
        print_trade_stats(&stats_2);
    }
    println!("-----------------------------------");
    Ok(())
}

async fn process_pair<D: ClickhouseDBMS>(
    clickhouse: &ClickhouseClient<D>,
    pair: Pair,
    start_timestamp: u64,
    end_timestamp: u64,
) -> Result<(), eyre::Report> {
    let pair_info = query_trading_pair_info(clickhouse, pair).await?;
    let stats =
        query_trade_stats(clickhouse, &pair_info.trading_pair, start_timestamp, end_timestamp)
            .await?;
    print_trade_stats(&stats);
    Ok(())
}

pub fn calculate_intermediary_addresses(
    trade_map: &FastHashMap<CexExchange, FastHashMap<Pair, Vec<CexTrades>>>,
    exchanges: &[CexExchange],
    pair: &Pair,
) -> FastHashSet<Address> {
    let (token_a, token_b) = (pair.0, pair.1);
    let mut connected_to_a = FastHashSet::new();
    let mut connected_to_b = FastHashSet::new();

    trade_map
        .iter()
        .filter(|(exchange, _)| exchanges.contains(exchange))
        .flat_map(|(_, pairs)| pairs.keys())
        .for_each(|trade_pair| {
            if trade_pair.0 == token_a {
                connected_to_a.insert(trade_pair.1);
            } else if trade_pair.1 == token_a {
                connected_to_a.insert(trade_pair.0);
            }

            if trade_pair.0 == token_b {
                connected_to_b.insert(trade_pair.1);
            } else if trade_pair.1 == token_b {
                connected_to_b.insert(trade_pair.0);
            }
        });

    connected_to_a
        .intersection(&connected_to_b)
        .cloned()
        .collect()
}

fn get_clickhouse_env() -> ClickhouseClient<NullDBMS> {
    let user = env::var("CLICKHOUSE_USER").expect("CLICKHOUSE_USER not set");
    let password = env::var("CLICKHOUSE_PASS").expect("CLICKHOUSE_PASS not set");
    let url = format!(
        "{}:{}",
        env::var("CLICKHOUSE_URL").expect("CLICKHOUSE_URL not set"),
        env::var("CLICKHOUSE_PORT").expect("CLICKHOUSE_PORT not set")
    );

    ClickhouseConfig::new(user, password, url, true, None).build()
}

async fn query_trading_pair_info<D: ClickhouseDBMS>(
    clickhouse: &ClickhouseClient<D>,
    pair: Pair,
) -> Result<TradingPairInfo, eyre::Report> {
    let result: TradingPairInfo = clickhouse
        .query_one(TRADING_PAIR_INFO_QUERY, &(pair.0.to_string(), pair.1.to_string()))
        .await?;

    Ok(result)
}

async fn query_trade_stats<D: ClickhouseDBMS>(
    clickhouse: &ClickhouseClient<D>,
    trading_pair: &str,
    start_timestamp: u64,
    end_timestamp: u64,
) -> Result<TradeStats, eyre::Report> {
    let result: TradeStats = clickhouse
        .query_one(TRADE_STATS_QUERY, &(trading_pair, start_timestamp, end_timestamp))
        .await?;

    Ok(result)
}

#[derive(Debug, Clone, Row, Deserialize, Serialize)]
struct TradeStats {
    symbol:        String,
    trade_count:   u64,
    total_volume:  f64,
    average_price: f64,
}

#[derive(Debug, Clone, Row, Deserialize, Serialize)]
struct TradingPairInfo {
    exchange:     String,
    trading_pair: String,
    base_asset:   (String, String),
    quote_asset:  (String, String),
}

fn print_trade_stats(stats: &TradeStats) {
    println!("Trade Statistics for {}", stats.symbol);
    println!("  Number of trades: {}", stats.trade_count);
    println!("  Total volume: {:.8}", stats.total_volume);
    println!("  Average price: {:.8}", stats.average_price);
}

// Define the SQL query as a constant
const TRADING_PAIR_INFO_QUERY: &str = "WITH
? AS address0,
? AS address1,
all_symbols AS (
    SELECT DISTINCT
    address,
    arrayJoin(CASE 
        WHEN unwrapped_symbol IS NOT NULL THEN [symbol, unwrapped_symbol]
        ELSE [symbol]
    END) AS symbol
    FROM cex.address_symbols WHERE address = address0 or address = address1
)
SELECT
s.exchange AS exchange,
s.pair AS trading_pair,
(p1.symbol, toString(p1.address)) AS base_asset,
(p2.symbol, toString(p2.address)) AS quote_asset
FROM cex.trading_pairs s
INNER JOIN all_symbols AS p1 ON p1.symbol = s.base_asset
INNER JOIN all_symbols AS p2 ON p2.symbol = s.quote_asset";

const TRADE_STATS_QUERY: &str = r#"
SELECT 
    symbol,
    COUNT(*) AS trade_count,
    SUM(amount) AS total_volume,
    SUM(price * amount) / SUM(amount) AS average_price
FROM 
    cex.normalized_trades
WHERE 
    symbol = ?
    AND timestamp BETWEEN ? AND ?
GROUP BY 
    symbol
"#;
