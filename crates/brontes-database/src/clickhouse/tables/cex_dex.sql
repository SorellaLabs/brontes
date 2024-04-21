CREATE TABLE mev.cex_dex ON CLUSTER eth_cluster0
(
    `tx_hash` String,
    `swaps` Nested(
        `trace_idx` UInt64,
        `from` String,
        `recipient` String,
        `pool` String,
        `token_in` String,
        `token_out` String,
        `amount_in` Tuple(UInt256, UInt256),
        `amount_out` Tuple(UInt256, UInt256)
    ),
    `global_vmap_details` Nested (
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
    `per_exchange_details` Nested (
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
    `last_updated` UInt64 DEFAULT now()
) 
ENGINE = ReplicatedReplacingMergeTree('/clickhouse/eth_cluster0/tables/all/mev/cex_dex', '{replica}', `last_updated`)
PRIMARY KEY (`tx_hash`)
ORDER BY (`tx_hash`)
