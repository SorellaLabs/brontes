CREATE TABLE mev.searcher_tx ON CLUSTER eth_cluster0
(
    `tx_hash` String,
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
    `last_updated` UInt64 DEFAULT now()
) 
ENGINE = ReplicatedReplacingMergeTree('/clickhouse/eth_cluster0/tables/all/mev/searcher_tx', '{replica}', `last_updated`)
PRIMARY KEY (`tx_hash`)
ORDER BY (`tx_hash`)


