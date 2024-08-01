CREATE TYPE token_pair_type AS (
    token_1 TEXT NOT NULL,
    token_2 TEXT NOT NULL
);

CREATE TYPE amount_pair_type AS (
    amount_1 uint256 NOT NULL,
    amount_2 uint256 NOT NULL
);

CREATE TYPE swaps_type AS (
    trace_idx BIGINT[] NOT NULL,
    from TEXT[] NOT NULL,
    recipient TEXT[] NOT NULL,
    pool TEXT[] NOT NULL,
    token_in token_pair_type[] NOT NULL,
    token_out token_pair_type[] NOT NULL,
    amount_in amount_pair_type[] NOT NULL,
    amount_out amount_pair_type[] NOT NULL
);

CREATE TYPE gas_details_type AS (
    coinbase_transfer UInt128,
    priority_fee UInt128,
    gas_used UInt128,
    effective_gas_price UInt128
);

CREATE TABLE mev.atomic_arbs (
    tx_hash VARCHAR(66) NOT NULL,
    block_number BIGINT NOT NULL,
    trigger_tx TEXT NOT NULL,
    swaps swaps_type NOT NULL,
    gas_details gas_details NOT NULL,
    arb_type TEXT NOT NULL,
    run_id BIGINT NOT NULL,
    PRIMARY KEY (block_number, tx_hash)
);