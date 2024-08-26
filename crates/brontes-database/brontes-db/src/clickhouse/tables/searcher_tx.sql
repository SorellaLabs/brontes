CREATE TABLE mev.searcher_tx ON CLUSTER eth_cluster0
(
    `tx_hash` String,
    `block_number` UInt64,
    `transfers` Nested(
        `trace_idx` UInt64,
        `from` String,
        `to` String,
        `pool` String,
        `token` Tuple(String, String),
        `amount` Tuple(UInt256, UInt256),
        `fee` Tuple(UInt256, UInt256),
        `msg_value` UInt256
    ),
    `gas_details` Tuple(Nullable(UInt128), UInt128, UInt128, UInt128),
    `run_id` UInt64
) 
ENGINE = ReplicatedMergeTree('/clickhouse/eth_cluster0/tables/all/mev/searcher_tx', '{replica}')
PRIMARY KEY (`block_number`,`tx_hash`)
ORDER BY (`block_number`, `tx_hash`)
