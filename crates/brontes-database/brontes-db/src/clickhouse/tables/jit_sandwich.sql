CREATE TABLE mev.jit_sandwich 
(
    `frontrun_tx_hash` String,
    `block_number` UInt64,
    `frontrun_swaps` Nested(
        `tx_hash` String,
        `trace_idx` UInt64,
        `from` String,
        `recipient` String,
        `pool` String,
        `token_in` Tuple(String, String),
        `token_out` Tuple(String, String),
        `amount_in` Tuple(UInt256, UInt256),
        `amount_out` Tuple(UInt256, UInt256)
    ),
    `frontrun_mints` Nested(
        `tx_hash` String,
        `trace_idx` UInt64,
        `from` String,
        `pool` String,
        `recipient` String,
        `tokens` Array(Tuple(String, String)),
        `amounts` Array(Tuple(UInt256, UInt256))
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
        `token_in` Tuple(String, String),
        `token_out` Tuple(String, String),
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
        `token_in` Tuple(String, String),
        `token_out` Tuple(String, String),
        `amount_in` Tuple(UInt256, UInt256),
        `amount_out` Tuple(UInt256, UInt256)
    ),
    `backrun_burns` Nested(
        `tx_hash` String,
        `trace_idx` UInt64,
        `from` String,
        `pool` String,
        `recipient` String,
        `tokens` Array(Tuple(String, String)),
        `amounts` Array(Tuple(UInt256, UInt256))
    ),
    `backrun_gas_details` Nested(
        `tx_hash` String,
        `coinbase_transfer` Nullable(UInt128), 
        `priority_fee` UInt128,
        `gas_used` UInt128,
        `effective_gas_price` UInt128
    ),
    `run_id` UInt64
) 
ENGINE = MergeTree()
PRIMARY KEY (`block_number`, `frontrun_tx_hash`)
ORDER BY (`block_number`, `frontrun_tx_hash` )
