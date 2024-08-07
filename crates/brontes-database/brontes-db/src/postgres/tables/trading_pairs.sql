CREATE TABLE cex.trading_pairs (
    exchange VARCHAR(32) NOT NULL,
    pair VARCHAR(32) NOT NULL,
    base_asset VARCHAR(32) NOT NULL,
    quote_asset VARCHAR(32) NOT NULL,
    trading_type VARCHAR(32) NOT NULL,
    last_updated TIMESTAMP NOT NULL DEFAULT NOW()
);
