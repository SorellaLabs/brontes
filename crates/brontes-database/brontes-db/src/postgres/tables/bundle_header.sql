CREATE TYPE TokenInfoWithAddressType AS (
    symbol      VARCHAR(20),
    decimals    SMALLINT,
    address     Hash256
);

CREATE TYPE TokenBalanceDeltaType AS (
    token       TokenInfoWithAddressType,
    amount      DOUBLE PRECISION,
    usd_value   DOUBLE PRECISION
);

CREATE TYPE AddressBalanceDeltaType AS (
    address         Hash256,
    name            TEXT,
    token_deltas    TokenBalanceDeltaType[]
);

CREATE TYPE TransactionAccountingType AS (
    tx_hash         Hash256,
    address_deltas  AddressBalanceDeltaType[]
);

CREATE TABLE mev.bundle_header (
    block_number BIGINT NOT NULL,
    tx_index BIGINT,
    tx_hash Hash256 NOT NULL,
    eoa Hash256 NOT NULL,
    mev_contract Hash256,
    fund Fund NOT NULL,
    profit_usd DOUBLE PRECISION NOT NULL,
    bribe_usd DOUBLE PRECISION NOT NULL,
    mev_type VARCHAR(32),
    no_pricing_calculated BOOLEAN DEFAULT false,
    balance_deltas TransactionAccountingType[] NOT NULL,
    run_id BIGINT NOT NULL,
    PRIMARY KEY (block_number, tx_hash)
);
