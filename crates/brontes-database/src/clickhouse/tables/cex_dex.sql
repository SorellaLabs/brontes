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
    `stat_arb_details` Nested (
        `cex_exchanges` Array(String),
        `cex_price` Tuple(UInt256, UInt256),
        `dex_exchange` String,
        `dex_price` Tuple(UInt256, UInt256),
        `pre_gas_maker_profit` Tuple(UInt256, UInt256),
        `pre_gas_taker_profit` Tuple(UInt256, UInt256)
    ),
    `pnl` Tuple(`taker_profit` Tuple(UInt256, UInt256), `maker_profit` Tuple(UInt256, UInt256)),
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
