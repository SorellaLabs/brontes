CREATE TABLE eth_analytics.private_txs (
    block_number BIGINT NOT NULL,
    tx_hash Hash256 NOT NULL,
    last_updated TIMESTAMP NOT NULL DEFAULT NOW(),
    PRIMARY KEY (block_number, tx_hash)
);
