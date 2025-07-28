CREATE TABLE IF NOT EXISTS cex.address_symbols (
    address FixedString(42),          -- Token contract address
    symbol LowCardinality(String),           -- Trading symbol (e.g., "BTC")
    unwrapped_symbol LowCardinality(String)  -- Optional unwrapped symbol (e.g., "WBTC" -> "BTC")
)
ENGINE = MergeTree()
PRIMARY KEY symbol
ORDER BY symbol
