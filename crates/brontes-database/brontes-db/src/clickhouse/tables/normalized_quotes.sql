CREATE TABLE cex.normalized_quotes (
    `exchange` LowCardinality(FixedString(16)),           -- The exchange name
    `symbol` LowCardinality(FixedString(16)),            -- The trading pair symbol (e.g., "BTC/USD", "ETH-BTC", etc.)
    `timestamp` Int64,          -- Timestamp in microseconds (Unix timestamp * 1000000)
    `ask_amount` Float64,       -- Amount available at ask price
    `ask_price` Float64,        -- Ask price
    `bid_price` Float64,        -- Bid price
    `bid_amount` Float64        -- Amount available at bid price
)
ENGINE = MergeTree()
PRIMARY KEY (timestamp, symbol)
ORDER BY (timestamp, symbol)