CREATE TABLE IF NOT EXISTS cex.address_symbols (
    chain VARCHAR(255) NOT NULL,
    exchange VARCHAR(255) NOT NULL,
    symbol VARCHAR(255) NOT NULL,
    name VARCHAR(255) NOT NULL,
    address VARCHAR(255) NOT NULL,
    unwrapped_symbol VARCHAR(255),
    last_updated TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL
);