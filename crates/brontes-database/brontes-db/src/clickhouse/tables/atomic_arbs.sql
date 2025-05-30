CREATE TABLE IF NOT EXISTS mev.atomic_arbs 
(
    `tx_hash` String,
    `block_number` UInt64,
    `trigger_tx` String,
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
    `gas_details` Tuple(
        `coinbase_transfer` Nullable(UInt128), 
        `priority_fee` UInt128,
        `gas_used` UInt128,
        `effective_gas_price` UInt128
    ),
    `arb_type` String,
    `run_id` UInt64
) 
ENGINE = MergeTree()
PRIMARY KEY (`block_number`, `tx_hash`)
ORDER BY (`block_number`, `tx_hash`)



