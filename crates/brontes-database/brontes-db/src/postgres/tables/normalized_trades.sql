CREATE TABLE IF NOT EXISTS cex.normalized_trades (
    exchange VARCHAR(32) NOT NULL,
    symbol VARCHAR(20) NOT NULL,
    timestamp TIMESTAMP NOT NULL,
    side VARCHAR(20) NOT NULL,
    price DOUBLE PRECISION NOT NULL,
    amount DOUBLE PRECISION NOT NULL,
    trade_id VARCHAR(255)
);
