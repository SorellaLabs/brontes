CREATE TABLE IF NOT EXISTS ethereum.pools     
(
    `protocol` LowCardinality(String),
    `protocol_subtype` LowCardinality(String),
    `address` FixedString(42),
    `tokens` Array(FixedString(42)),
    `curve_lp_token` Nullable(FixedString(42)),
    `init_block` UInt64
) 
ENGINE = ReplacingMergeTree()
ORDER BY (`address`)