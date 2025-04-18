CREATE TABLE cex.trading_pairs (
    exchange LowCardinality(String),         -- Exchange name
    pair LowCardinality(String),             -- Trading pair identifier
    base_asset LowCardinality(String),       -- Symbol of base asset
    quote_asset LowCardinality(String)       -- Symbol of quote asset
)
ENGINE = MergeTree()
PRIMARY KEY (exchange, pair)
ORDER BY (exchange, pair)