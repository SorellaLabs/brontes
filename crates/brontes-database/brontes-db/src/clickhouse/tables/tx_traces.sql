CREATE TABLE brontes_api.tx_traces 
(
    `block_number` Uint64,
    `tx_hash` String,
    `traces` Blob,
    `gas_used` Uint64,
    `effective_price` Uint64,
    `tx_index` Uint64,
    `is_success` Boolean,
    `created_at` Timestamp,
)
ENGINE = MergeTree()
PRIMARY KEY (block_number, tx_hash)
ORDER BY (block_number, tx_hash)