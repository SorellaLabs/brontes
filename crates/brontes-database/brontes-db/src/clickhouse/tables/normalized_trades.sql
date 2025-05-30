CREATE TABLE IF NOT EXISTS cex.normalized_trades (
    `symbol` LowCardinality(String),           -- Trading pair symbol (e.g., "BTC/USDT")
    `exchange` LowCardinality(String),         -- Exchange name
    `side` LowCardinality(String),            -- Side of the trade (e.g., "buy", "sell")
    `timestamp` UInt64,        -- Unix timestamp in microseconds
    `amount` Float64,          -- Trade amount/volume
    `price` Float64            -- Trade price
)
ENGINE = MergeTree()
PRIMARY KEY (timestamp, symbol)
ORDER BY (timestamp, symbol)