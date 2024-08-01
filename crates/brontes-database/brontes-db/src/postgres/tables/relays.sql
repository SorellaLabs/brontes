CREATE TABLE ethereum.relays (
    epoch INTEGER NOT NULL,
    slot INTEGER NOT NULL,
    block_number INTEGER NOT NULL,
    parent_hash Hash256 NOT NULL,
    block_hash Hash256 NOT NULL,
    relay VARCHAR(64) NOT NULL,
    builder_pubkey CHAR(98) NOT NULL,
    proposer_fee_recipient CHAR(42) NOT NULL,
    gas_limit BIGINT NOT NULL,
    gas_used BIGINT NOT NULL,
    value NUMERIC(39,0) NOT NULL,
    tx_num SMALLINT NOT NULL,
    was_accepted BOOLEAN NOT NULL,
    timestamp TIMESTAMP NOT NULL
);