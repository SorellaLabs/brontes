CREATE TABLE mev.jit ON CLUSTER eth_cluster0
(
    `frontrun_mint_tx_hash` String,
    `frontrun_mints` Nested(
        `trace_idx` UInt64,
        `from` String,
        `pool` String,
        `recipient` String,
        `tokens` Array(Tuple(String, String)),
        `amounts` Array(Tuple(UInt256, UInt256))
    ),
    `frontrun_mint_gas_details` Tuple(
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
    `backrun_burn_tx_hash` String,
    `backrun_burns` Nested(
        `trace_idx` UInt64,
        `from` String,
        `pool` String,
        `recipient` String,
        `tokens` Array(Tuple(String, String)),
        `amounts` Array(Tuple(UInt256, UInt256))
    ),
    `backrun_burn_gas_details` Tuple(
        `coinbase_transfer` Nullable(UInt128), 
        `priority_fee` UInt128,
        `gas_used` UInt128,
        `effective_gas_price` UInt128
    ),
    `last_updated` UInt64 DEFAULT now()
) 
ENGINE = ReplicatedReplacingMergeTree('/clickhouse/eth_cluster0/tables/all/mev/jit', '{replica}', `last_updated`)
PRIMARY KEY (`backrun_burn_tx_hash`)
ORDER BY (`backrun_burn_tx_hash`)