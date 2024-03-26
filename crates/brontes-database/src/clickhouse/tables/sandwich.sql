CREATE TABLE mev.sandwiches ON CLUSTER eth_cluster0
(
    `frontrun_tx_hash` String,
    `frontrun_swaps` Nested(
        `tx_hash` String,
        `trace_idx` UInt64,
        `from` String,
        `recipient` String,
        `pool` String,
        `token_in` String,
        `token_out` String,
        `amount_in` Tuple(UInt256, UInt256),
        `amount_out` Tuple(UInt256, UInt256)
    ),
    `frontrun_gas_details` Nested(
        `tx_hash` String,
        `coinbase_transfer` Nullable(UInt128), 
        `priority_fee` UInt128,
        `gas_used` UInt128,
        `effective_gas_price` UInt128
    ),
    `victim_swaps` Nested(
        `tx_hash` String,
        `trace_idx` UInt64,
        `from` String,
        `recipient` String,
        `pool` String,
        `token_in` String,
        `token_out` String,
        `amount_in` Tuple(UInt256, UInt256),
        `amount_out` Tuple(UInt256, UInt256)
    ),
    `victim_gas_details` Nested(
        `tx_hash` String,
        `coinbase_transfer` Nullable(UInt128), 
        `priority_fee` UInt128,
        `gas_used` UInt128,
        `effective_gas_price` UInt128
    ),
    `backrun_tx_hash` String,
    `backrun_swaps` Nested(
        `tx_hash` String,
        `trace_idx` UInt64,
        `from` String,
        `recipient` String,
        `pool` String,
        `token_in` String,
        `token_out` String,
        `amount_in` Tuple(UInt256, UInt256),
        `amount_out` Tuple(UInt256, UInt256)
    ),
    `backrun_gas_details` Nested(
        `tx_hash` String,
        `coinbase_transfer` Nullable(UInt128), 
        `priority_fee` UInt128,
        `gas_used` UInt128,
        `effective_gas_price` UInt128
    ),
    `last_updated` UInt64 DEFAULT now()
) 
ENGINE = ReplicatedReplacingMergeTree('/clickhouse/eth_cluster0/tables/all/mev/sandwiches', '{replica}', `last_updated`)
PRIMARY KEY (`frontrun_tx_hash`)
ORDER BY (`frontrun_tx_hash`)