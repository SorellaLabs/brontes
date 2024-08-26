CREATE TABLE mev.bundle_header ON CLUSTER eth_cluster0
(
    `block_number` UInt64,
    `tx_index` UInt64,
    `tx_hash` String,
    `eoa` String,
    `mev_contract` Nullable(String),
    `fund` String,
    `profit_usd` Float64,
    `bribe_usd` Float64,
    `mev_type` String,
    `no_pricing_calculated` Bool DEFAULT false,
    `balance_deltas` Nested (
        `tx_hash` String,
        `address` String,
        `name` Nullable(String),
        `token_deltas` Array(Tuple(Tuple(String, UInt8, String), Float64, Float64))
    ),
    `run_id` UInt64
) 
ENGINE = ReplicatedMergeTree('/clickhouse/eth_cluster0/tables/all/mev/bundle_header', '{replica}')
PRIMARY KEY (`block_number`, `tx_hash`)
ORDER BY (`block_number`, `tx_hash`)