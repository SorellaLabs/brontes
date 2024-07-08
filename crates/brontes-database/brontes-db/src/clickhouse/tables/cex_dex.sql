CREATE TABLE mev.cex_dex ON CLUSTER eth_cluster0
(
    `tx_hash` String,
    `block_number` UInt64,
    `swaps` Nested(
        `trace_idx` UInt64,
        `from` String,
        `recipient` String,
        `pool` String,
        `token_in` Tuple(String, String),
        `token_out` Tuple(String, String),
        `amount_in` Tuple(UInt256, UInt256),
        `amount_out` Tuple(UInt256, UInt256)
    ),
    `global_vmap_details` Nested (
        `pairs` Array(Tuple(String, String)),
        `trade_start_time` UInt64,
        `trade_end_time` UInt64,
        `cex_exchange` String,
        `best_bid_maker` Tuple(UInt256, UInt256),
        `best_ask_maker` Tuple(UInt256, UInt256),
        `best_bid_taker` Tuple(UInt256, UInt256),
        `best_ask_taker` Tuple(UInt256, UInt256),
        `dex_exchange` String,
        `dex_price` Tuple(UInt256, UInt256),
        `dex_amount` Tuple(UInt256, UInt256),
        `pnl_pre_gas` Tuple(`maker_taker_mid` Tuple(Tuple(UInt256, UInt256), Tuple(UInt256, UInt256)), `maker_taker_ask` Tuple(Tuple(UInt256, UInt256),Tuple(UInt256, UInt256)))
    ),
    `global_vmap_pnl` Tuple(`maker_taker_mid` Tuple(Tuple(UInt256, UInt256),Tuple(UInt256, UInt256)), `maker_taker_ask` Tuple(Tuple(UInt256, UInt256),Tuple(UInt256, UInt256))),
    `optimal_route_details` Nested (
        `pairs` Array(Tuple(String, String)),
        `trade_start_time` UInt64,
        `trade_end_time` UInt64,
        `cex_exchange` String,
        `best_bid_maker` Tuple(UInt256, UInt256),
        `best_ask_maker` Tuple(UInt256, UInt256),
        `best_bid_taker` Tuple(UInt256, UInt256),
        `best_ask_taker` Tuple(UInt256, UInt256),
        `dex_exchange` String,
        `dex_price` Tuple(UInt256, UInt256),
        `dex_amount` Tuple(UInt256, UInt256),
        `pnl_pre_gas` Tuple(`maker_taker_mid` Tuple(Tuple(UInt256, UInt256),Tuple(UInt256, UInt256)), `maker_taker_ask` Tuple(Tuple(UInt256, UInt256),Tuple(UInt256, UInt256)))
    ),
    `optimal_route_pnl` Tuple(`maker_taker_mid` Tuple(Tuple(UInt256, UInt256),Tuple(UInt256, UInt256)), `maker_taker_ask` Tuple(Tuple(UInt256, UInt256),Tuple(UInt256, UInt256))),
    `optimistic_route_details` Nested (
        `pairs` Array(Tuple(String, String)),
        `trade_start_time` UInt64,
        `trade_end_time` UInt64,
        `cex_exchange` String,
        `best_bid_maker` Tuple(UInt256, UInt256),
        `best_ask_maker` Tuple(UInt256, UInt256),
        `best_bid_taker` Tuple(UInt256, UInt256),
        `best_ask_taker` Tuple(UInt256, UInt256),
        `dex_exchange` String,
        `dex_price` Tuple(UInt256, UInt256),
        `dex_amount` Tuple(UInt256, UInt256),
        `pnl_pre_gas` Tuple(`maker_taker_mid` Tuple(Tuple(UInt256, UInt256), Tuple(UInt256, UInt256)), `maker_taker_ask` Tuple(Tuple(UInt256, UInt256),Tuple(UInt256, UInt256)))
    ),
    `optimistic_trade_details` Array(Array(Tuple(`exchange` String, `pair` Tuple(String, String), `timestamp` UInt64, `price` Float64, `volume` Float64))),
    `optimistic_route_pnl` Tuple(`maker_taker_mid` Tuple(Tuple(Nullable(UInt256), Nullable(UInt256)),Tuple(Nullable(UInt256), Nullable(UInt256))),`maker_taker_ask` Tuple(Tuple(Nullable(UInt256), Nullable(UInt256)),Tuple(Nullable(UInt256), Nullable(UInt256)))),
    `global_time_window_start` UInt64,
    `global_time_window_end`   UInt64,
    `global_optimistic_start`  UInt64,
    `global_optimistic_end`    UInt64,
    `per_exchange_details` Nested (
        `pairs` Array(Array(Tuple(String, String))),
        `trade_start_time` Array(UInt64),
        `trade_end_time` Array(UInt64),
        `cex_exchange` Array(String),
        `best_bid_maker` Array(Tuple(UInt256, UInt256)),
        `best_ask_maker` Array(Tuple(UInt256, UInt256)),
        `best_bid_taker` Array(Tuple(UInt256, UInt256)),
        `best_ask_taker` Array(Tuple(UInt256, UInt256)),
        `dex_exchange` Array(String),
        `dex_price` Array(Tuple(UInt256, UInt256)),
        `dex_amount` Array(Tuple(UInt256, UInt256)),
        `pnl_pre_gas` Array(Tuple(`maker_taker_mid` Tuple(Tuple(UInt256, UInt256),Tuple(UInt256, UInt256)), `maker_taker_ask` Tuple(Tuple(UInt256, UInt256),Tuple(UInt256, UInt256))))
    ),
    `per_exchange_pnl` Nested (
        `cex_exchange` String,
        `arb_pnl` Tuple(`maker_taker_mid` Tuple(Tuple(UInt256, UInt256),Tuple(UInt256, UInt256)), `maker_taker_ask` Tuple(Tuple(UInt256, UInt256),Tuple(UInt256, UInt256)))
    ),
    `gas_details` Tuple(
        `coinbase_transfer` Nullable(UInt128), 
        `priority_fee` UInt128,
        `gas_used` UInt128,
        `effective_gas_price` UInt128
    ),
    `run_id` UInt64
) 
ENGINE = ReplicatedReplacingMergeTree('/clickhouse/eth_cluster0/tables/all/mev/cex_dex', '{replica}', `run_id`)
PRIMARY KEY (`block_number`, `tx_hash`)
ORDER BY (`block_number`, `tx_hash`)

