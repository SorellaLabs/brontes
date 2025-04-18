CREATE TABLE cex.address_symbols (
    address String,          -- Token contract address
    symbol String,           -- Trading symbol (e.g., "BTC")
    unwrapped_symbol String  -- Optional unwrapped symbol (e.g., "WBTC" -> "BTC")
)
ENGINE = MergeTree()
PRIMARY KEY symbol
ORDER BY symbol
