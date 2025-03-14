use ahash::HashSetExt;
use alloy_primitives::Address;
use brontes_database::{clickhouse::cex_config::CexDownloadConfig, libmdbx::LibmdbxReader};
use brontes_types::{
    constants::USDT_ADDRESS,
    db::cex::{trades::CexTrades, CexExchange},
    init_thread_pools,
    pair::Pair,
    FastHashMap, FastHashSet,
};
use clap::Parser;
use clickhouse::Row;
use db_interfaces::{
    clickhouse::{
        client::ClickhouseClient,
        dbms::{ClickhouseDBMS, NullDBMS},
    },
    errors::DatabaseError,
    Database,
};
use eyre::Result;
use prettytable::{Cell, Row, Table};
use serde::{Deserialize, Serialize};

use super::utils::get_clickhouse_env;
use crate::{
    cli::{load_libmdbx, static_object},
    runner::CliContext,
};
const SECONDS_TO_US: u64 = 1_000_000;

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
    /// Time window multiplier (expands it)
    #[arg(long, short, default_value_t = 1.0)]
    pub w_multiplier: f64,
}

impl CexDB {
    pub async fn execute(self, brontes_db_path: String, ctx: CliContext) -> eyre::Result<()> {
        init_thread_pools(10);

        let task_executor = ctx.task_executor;

        let cex_config = CexDownloadConfig::default();

        let libmdbx = static_object(load_libmdbx(&task_executor, brontes_db_path)?);

        let metadata = libmdbx.get_metadata(self.block_number, USDT_ADDRESS)?;

        let clickhouse: ClickhouseClient<NullDBMS> = get_clickhouse_env();

        let token0: Address = self.token_0.parse()?;
        let token1: Address = self.token_1.parse()?;

        let pair = Pair(token0, token1);

        let block_timestamp = metadata.microseconds_block_timestamp();

        let cex_trades = &metadata.cex_trades.as_ref().unwrap().0;
        let exchanges_to_use = &cex_config.exchanges_to_use;

        let pair_exists = exchanges_to_use.iter().any(|exchange| {
            cex_trades
                .get(exchange)
                .is_some_and(|pairs| pairs.contains_key(&pair) || pairs.contains_key(&pair.flip()))
        });

        if !pair_exists {
            println!("No direct trading pair found for {:?}", pair);
        } else {
            process_pair(&clickhouse, pair, block_timestamp, (10.0 * self.w_multiplier) as u64)
                .await?;
        }

        let intermediary_addresses =
            calculate_intermediary_addresses(cex_trades, &cex_config.exchanges_to_use, &pair);

        println!("Found {} intermediary addresses", intermediary_addresses.len());

        process_intermediaries(
            &clickhouse,
            pair,
            intermediary_addresses,
            block_timestamp,
            (10.0_f64 * self.w_multiplier) as u64,
        )
        .await?;

        Ok(())
    }
}

async fn process_intermediaries<D: ClickhouseDBMS>(
    clickhouse: &ClickhouseClient<D>,
    pair: Pair,
    intermediaries: FastHashSet<Address>,
    block_timestamp: u64,
    tw_size: u64,
) -> Result<(), eyre::Report> {
    for intermediary in intermediaries {
        let intermediary_pair_1 = Pair(pair.0, intermediary);
        let intermediary_pair_2 = Pair(intermediary, pair.1);

        println!("Processing intermediary: {:?}", intermediary);

        // Query for the first intermediary pair
        let pair_info_1 = query_trading_pair_info(clickhouse, intermediary_pair_1).await?;

        query_trade_stats(clickhouse, &pair_info_1.trading_pair, block_timestamp, tw_size).await?;

        // Query for the second intermediary pair
        let pair_info_2 = query_trading_pair_info(clickhouse, intermediary_pair_2).await?;

        query_trade_stats(clickhouse, &pair_info_2.trading_pair, block_timestamp, tw_size).await?;
    }
    println!("-----------------------------------");
    Ok(())
}

async fn process_pair<D: ClickhouseDBMS>(
    clickhouse: &ClickhouseClient<D>,
    pair: Pair,
    block_timestamp: u64,
    tw_size: u64,
) -> Result<(), eyre::Report> {
    let pair_info = query_trading_pair_info(clickhouse, pair).await?;

    query_trade_stats(clickhouse, &pair_info.trading_pair, block_timestamp, tw_size).await?;

    Ok(())
}

async fn query_trade_stats<D: ClickhouseDBMS>(
    clickhouse: &ClickhouseClient<D>,
    trading_pair: &str,
    block_timestamp: u64,
    tw_size: u64,
) -> Result<(), eyre::Report> {
    println!("Querying trade stats for {}", trading_pair);

    let start_time = block_timestamp - tw_size * SECONDS_TO_US;
    let end_time = block_timestamp + tw_size * SECONDS_TO_US;

    let result: Result<Vec<TradeStats>, DatabaseError> = clickhouse
        .query_many(TRADE_STATS_QUERY, &(block_timestamp, start_time, end_time, trading_pair))
        .await;

    match result {
        Ok(stats) => print_trade_stats(&stats),
        Err(e) => {
            println!("No trades for {} stats: {:?}", trading_pair, e);
        }
    }

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

async fn query_trading_pair_info<D: ClickhouseDBMS>(
    clickhouse: &ClickhouseClient<D>,
    pair: Pair,
) -> Result<TradingPairInfo, eyre::Report> {
    let result: TradingPairInfo = clickhouse
        .query_one(
            TRADING_PAIR_INFO_QUERY,
            &(pair.0.to_string().to_lowercase(), pair.1.to_string().to_lowercase()),
        )
        .await?;

    Ok(result)
}

#[derive(Debug, Clone, Row, Deserialize, Serialize)]
struct TradeStats {
    symbol:             String,
    exchange:           String,
    period:             String,
    seconds_from_block: i64,
    trade_count:        u64,
    total_volume:       f64,
    average_price:      f64,
}
fn print_trade_stats(stats: &[TradeStats]) {
    if stats.is_empty() {
        return;
    }

    let symbol = &stats[0].symbol;
    println!("Trade Statistics for {}", symbol);

    let mut before_table = Table::new();
    let mut after_table = Table::new();
    for table in [&mut before_table, &mut after_table].iter_mut() {
        table.add_row(Row::new(vec![
            Cell::new("Seconds"),
            Cell::new("Exchange"),
            Cell::new("Trade Count"),
            Cell::new("Volume"),
            Cell::new("Avg Price"),
        ]));
    }

    let mut total_volume = 0.0;
    let mut volume_by_exchange: std::collections::HashMap<String, f64> =
        std::collections::HashMap::new();

    for stat in stats {
        total_volume += stat.total_volume;
        *volume_by_exchange.entry(stat.exchange.clone()).or_default() += stat.total_volume;

        let table = if stat.period == "before" { &mut before_table } else { &mut after_table };

        table.add_row(Row::new(vec![
            Cell::new(&format!("{}-{}", stat.seconds_from_block, stat.seconds_from_block + 1)),
            Cell::new(&stat.exchange),
            Cell::new(&stat.trade_count.to_string()),
            Cell::new(&format!("{:.8}", stat.total_volume)),
            Cell::new(&format!("{:.8}", stat.average_price)),
        ]));
    }

    println!("\nTrades before block time:");
    before_table.printstd();
    println!("\nTrades after block time:");
    after_table.printstd();

    println!("\nTotal volume across all exchanges: {:.8}", total_volume);
    println!("Volume breakdown by exchange:");
    for (exchange, volume) in volume_by_exchange.iter() {
        println!("  {}: {:.8} ({:.2}%)", exchange, volume, (volume / total_volume) * 100.0);
    }
}

const TRADE_STATS_QUERY: &str = r#"
WITH 
    ? AS block_time,
    ? AS start_time,
    ? AS end_time,
    ? AS symbol_param,
    trades_in_time AS (
        SELECT 
            symbol,
            exchange,
            IF(timestamp < block_time, 'before', 'after') AS period,
            IF(timestamp < block_time,
                CAST((block_time - timestamp) / 1000000, 'Int64'),
                CAST((timestamp - block_time) / 1000000, 'Int64')
             ) AS seconds_from_block,
            amount,
            price
        FROM 
            cex.normalized_trades
        WHERE 
            symbol = symbol_param
            AND timestamp BETWEEN start_time AND end_time
    )
SELECT 
    symbol,
    exchange,
    period,
    seconds_from_block,
    COUNT(*) AS trade_count,
    SUM(amount) AS total_volume,
    SUM(price * amount) / SUM(amount) AS average_price
FROM trades_in_time
GROUP BY 
    symbol, exchange, period, seconds_from_block
ORDER BY
    period, seconds_from_block
"#;

#[derive(Debug, Clone, Row, Deserialize, Serialize)]
struct TradingPairInfo {
    exchange:     String,
    trading_pair: String,
    base_asset:   (String, String),
    quote_asset:  (String, String),
}
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
FROM cex.trading_pairs AS s
INNER JOIN all_symbols AS p1 ON p1.symbol = s.base_asset
INNER JOIN all_symbols AS p2 ON p2.symbol = s.quote_asset";
