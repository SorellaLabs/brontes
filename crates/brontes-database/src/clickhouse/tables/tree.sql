CREATE TABLE brontes.tree ON CLUSTER eth_cluster0
(
    `tx_hash` String,
    `tx_idx` UInt64,
    `gas_details` Tuple(
        `coinbase_transfer` Nullable(UInt128), 
        `priority_fee` UInt128,
        `gas_used` UInt128,
        `effective_gas_price` UInt128
    ),
    `trace_nodes` Nested(
        `trace_idx` UInt64,
        `trace_address` Array(UInt64),
        `action_kind` Nullable(String),
        `action` Nullable(String)
    ),
    `last_updated` UInt64 DEFAULT now()
) 
ENGINE = ReplicatedReplacingMergeTree('/clickhouse/eth_cluster0/tables/all/brontes/tree', '{replica}', `last_updated`)
PRIMARY KEY `tx_hash`
ORDER BY `tx_hash` 
