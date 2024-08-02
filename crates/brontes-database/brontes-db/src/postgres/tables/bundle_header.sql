CREATE TABLE mev.bundle_header (
    block_number BIGINT,
    tx_index BIGINT,
    tx_hash TEXT,
    eoa TEXT,
    mev_contract TEXT,
    fund TEXT,
    profit_usd DOUBLE PRECISION,
    bribe_usd DOUBLE PRECISION,
    mev_type TEXT,
    no_pricing_calculated BOOLEAN DEFAULT false,
    balance_deltas JSONB,
    run_id BIGINT,
    PRIMARY KEY (block_number, tx_hash)
);

CREATE INDEX idx_bundle_header_run_id ON mev.bundle_header (run_id);
