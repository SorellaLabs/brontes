CREATE TABLE ethereum.blocks (
    block_number BIGINT NOT NULL,
    block_hash Hash256 NOT NULL,
    parent_hash Hash256 NOT NULL,
    fee_recipient CHAR(42) NOT NULL,
    state_root Hash256 NOT NULL,
    receipts_root Hash256 NOT NULL,
    logs_bloom TEXT NOT NULL,
    mixed_hash Hash256 NOT NULL,
    extra_data TEXT NOT NULL,
    gas_limit BIGINT NOT NULL,
    gas_used BIGINT NOT NULL,
    base_fee_per_gas BIGINT,
    block_timestamp TIMESTAMP NOT NULL,
    transaction_hashes Hash256[] NOT NULL,
    valid SMALLINT NOT NULL CHECK (valid >= 0 AND valid <= 255)
);
