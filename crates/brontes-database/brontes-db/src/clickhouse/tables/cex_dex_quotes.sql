CREATE TABLE mev.cex_dex_quotes ON CLUSTER eth_cluster0
(
    `tx_hash` String,
    `block_timestamp` UInt64,
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
    `instant_mid_price` Array(Float64),
    `t2_mid_price` Array(Float64),
    `t12_mid_price` Array(Float64),
    `t30_mid_price` Array(Float64),
    `t60_mid_price` Array(Float64),
    `t300_mid_price` Array(Float64),
    `exchange` String,
    `pnl` Float64,
    `gas_details` Tuple(
        `coinbase_transfer` Nullable(UInt128),
        `priority_fee` UInt128,
        `gas_used` UInt128,
        `effective_gas_price` UInt128
    ),
    `run_id` UInt64
)
ENGINE = ReplicatedMergeTree('/clickhouse/eth_cluster0/tables/all/mev/mev_cex_dex_quotes', '{replica}')
PRIMARY KEY (`block_number`, `tx_hash`)
ORDER BY (`block_number`, `tx_hash`)