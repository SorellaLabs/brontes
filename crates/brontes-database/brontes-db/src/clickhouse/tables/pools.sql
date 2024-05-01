CREATE TABLE ethereum.pools ON CLUSTER eth_cluster0
(
    `protocol` LowCardinality(String),
    `protocol_subtype` LowCardinality(String),
    `address` FixedString(42),
    `tokens` Array(FixedString(42)),
    `curve_lp_token` Nullable(FixedString(42)),
    `init_block` UInt64
) 
ENGINE = ReplicatedReplacingMergeTree('/clickhouse/eth_cluster0/tables/all/eth/pools', '{replica}', `init_block`)
PRIMARY KEY (`protocol`, `address`)
ORDER BY (`protocol`, `address`)
