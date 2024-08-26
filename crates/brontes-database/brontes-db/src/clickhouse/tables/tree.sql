CREATE TABLE brontes.tree ON CLUSTER eth_cluster0
(
    `block_number` UInt64,
    `tx_hash` String,
    `tx_idx` UInt64,
    `from` String,
    `to` Nullable(String),
    `gas_details` Tuple(coinbase_transfer Nullable(UInt128), priority_fee UInt128, gas_used UInt128, effective_gas_price UInt128),
    `trace_nodes.trace_idx` Array(UInt64),
    `trace_nodes.trace_address` Array(Array(UInt64)),
    `trace_nodes.action_kind` Array(Nullable(String)),
    `trace_nodes.action` Array(Nullable(String)),
    `run_id` UInt64
)
ENGINE = ReplicatedMergeTree('/clickhouse/eth_cluster0/tables/all/brontes/tree', '{replica}')
PRIMARY KEY (`block_number`, `tx_hash`)
ORDER BY (`block_number`, `tx_hash`)
SETTINGS index_granularity = 8192, parts_to_throw_insert = 10000