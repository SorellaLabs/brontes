CREATE TABLE IF NOT EXISTS ethereum.blocks (
    `block_number` UInt64,
    `block_hash` String,
    `block_timestamp` UInt64,
    `valid` UInt8,
) ENGINE = MergeTree()
ORDER BY block_number