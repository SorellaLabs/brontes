CREATE TABLE mev.cex_dex 
(
    `tx_hash` String,
    `block_timestamp` UInt64,
    `block_number` UInt64,
    `header_pnl_methodology` String,
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
    `global_vmap_details` Nested(
        `pairs` Array(Tuple(String, String)),
        `trade_start_time` UInt64,
        `trade_end_time` UInt64,
        `cex_exchange` String,
        `price_maker` Tuple(UInt256, UInt256),
        `price_taker` Tuple(UInt256, UInt256),
        `dex_exchange` String,
        `dex_price` Tuple(UInt256, UInt256),
        `dex_amount` Tuple(UInt256, UInt256),
        `pnl_maker` Tuple(UInt256, UInt256),
        `pnl_taker` Tuple(UInt256, UInt256)
    ),
    `global_vmap_pnl_maker` Tuple(UInt256, UInt256),
    `global_vmap_pnl_taker` Tuple(UInt256, UInt256),
    `optimal_route_details` Nested(
        `pairs` Array(Tuple(String, String)),
        `trade_start_time` UInt64,
        `trade_end_time` UInt64,
        `cex_exchange` String,
        `price_maker` Tuple(UInt256, UInt256),
        `price_taker` Tuple(UInt256, UInt256),
        `dex_exchange` String,
        `dex_price` Tuple(UInt256, UInt256),
        `dex_amount` Tuple(UInt256, UInt256),
        `pnl_maker` Tuple(UInt256, UInt256),
        `pnl_taker` Tuple(UInt256, UInt256)
    ),
    `optimal_route_pnl_maker` Tuple(UInt256, UInt256),
    `optimal_route_pnl_taker` Tuple(UInt256, UInt256),
    `optimistic_route_details` Nested(
        `pairs` Array(Tuple(String, String)),
        `trade_start_time` UInt64,
        `trade_end_time` UInt64,
        `cex_exchange` String,
        `price_maker` Tuple(UInt256, UInt256),
        `price_taker` Tuple(UInt256, UInt256),
        `dex_exchange` String,
        `dex_price` Tuple(UInt256, UInt256),
        `dex_amount` Tuple(UInt256, UInt256),
        `pnl_maker` Tuple(UInt256, UInt256),
        `pnl_taker` Tuple(UInt256, UInt256)
    ),
    `optimistic_trade_details` Array(Array(Tuple(
        `exchange` String,
        `pair` Tuple(String, String),
        `timestamp` UInt64,
        `price` Tuple(UInt256, UInt256),
        `volume` Tuple(UInt256, UInt256)
    ))),
    `optimistic_route_pnl_maker` Tuple(UInt256, UInt256),
    `optimistic_route_pnl_taker` Tuple(UInt256, UInt256),
    `per_exchange_details` Nested(
        `pairs` Array(Array(Tuple(String, String))),
        `trade_start_time` Array(UInt64),
        `trade_end_time` Array(UInt64),
        `cex_exchange` Array(String),
        `price_maker` Array(Tuple(UInt256, UInt256)),
        `price_taker` Array(Tuple(UInt256, UInt256)),
        `dex_exchange` Array(String),
        `dex_price` Array(Tuple(UInt256, UInt256)),
        `dex_amount` Array(Tuple(UInt256, UInt256)),
        `pnl_maker` Array(Tuple(UInt256, UInt256)),
        `pnl_taker` Array(Tuple(UInt256, UInt256))
    ),
    `per_exchange_pnl` Nested(
        `cex_exchange` String,
        `pnl_maker` Tuple(UInt256, UInt256),
        `pnl_taker` Tuple(UInt256, UInt256)
    ),
    `gas_details` Tuple(
        `coinbase_transfer` Nullable(UInt128),
        `priority_fee` UInt128,
        `gas_used` UInt128,
        `effective_gas_price` UInt128
    ),
    `run_id` UInt64
)
ENGINE = MergeTree()
PRIMARY KEY (block_number, tx_hash)
ORDER BY (block_number, tx_hash)
