CREATE TABLE ethereum.pools     
(
    `protocol` LowCardinality(String),
    `protocol_subtype` LowCardinality(String),
    `address` FixedString(42),
    `tokens` Array(FixedString(42)),
    `curve_lp_token` Nullable(FixedString(42)),
    `init_block` UInt64
) 
ENGINE = MergeTree()
PRIMARY KEY (`protocol`, `address`)
ORDER BY (`protocol`, `address`)
