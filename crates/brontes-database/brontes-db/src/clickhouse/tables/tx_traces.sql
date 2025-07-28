CREATE TABLE IF NOT EXISTS brontes_api.tx_traces 
(
    `block_number` UInt64,
    `tx_hash` String,
    `traces` Blob,
    `gas_used` UInt64,
    `effective_price` UInt64,
    `tx_index` UInt64,
    `is_success` Boolean,
    `created_at` Timestamp,
)
ENGINE = MergeTree()
PRIMARY KEY (block_number, tx_hash)
ORDER BY (block_number, tx_hash)