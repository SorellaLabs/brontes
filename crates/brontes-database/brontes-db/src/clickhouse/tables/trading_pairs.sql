CREATE TABLE IF NOT EXISTS cex.trading_pairs (
    exchange LowCardinality(String),         -- Exchange name
    trading_type LowCardinality(String),     -- Trading type
    pair LowCardinality(String),             -- Trading pair identifier
    base_asset LowCardinality(String),       -- Symbol of base asset
    quote_asset LowCardinality(String)       -- Symbol of quote asset
)
ENGINE = MergeTree()
PRIMARY KEY (exchange, pair)
ORDER BY (exchange, pair)