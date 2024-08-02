CREATE TABLE mev.atomic_arbs (
    tx_hash TEXT,
    block_number BIGINT,
    trigger_tx TEXT,
    swaps JSONB,
    gas_details JSONB,
    run_id BIGINT,
    PRIMARY KEY (block_number, tx_hash)
);

CREATE INDEX idx_atomic_arbs_block_tx ON mev.atomic_arbs (block_number, tx_hash);
