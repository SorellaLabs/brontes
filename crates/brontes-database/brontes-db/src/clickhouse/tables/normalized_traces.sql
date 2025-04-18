CREATE TABLE cex.normalized_trades (
    symbol String,           -- Trading pair symbol (e.g., "BTC/USDT")
    exchange String,         -- Exchange name
    timestamp UInt64,        -- Unix timestamp in microseconds
    amount Float64,          -- Trade amount/volume
    price Float64            -- Trade price
)
ENGINE = MergeTree()
PRIMARY KEY symbol
ORDER BY timestamp
