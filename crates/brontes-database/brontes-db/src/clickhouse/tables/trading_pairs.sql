CREATE TABLE cex.trading_pairs (
    exchange String,         -- Exchange name
    pair String,             -- Trading pair identifier
    base_asset String,       -- Symbol of base asset
    quote_asset String       -- Symbol of quote asset
)
ENGINE = MergeTree()
PRIMARY KEY exchange, pair
ORDER BY exchange, pair