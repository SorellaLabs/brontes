CREATE TABLE brontes_api.builder_info (
    address Hash256 NOT NULL,
    name TEXT,
    fund VARCHAR(32),
    pub_keys Fund[] NOT NULL,
    searchers_eoas Hash256[] NOT NULL,
    searchers_contracts Hash256[] NOT NULL,
    ultrasound_relay_collateral_address Hash256,
    last_updated TIMESTAMP NOT NULL DEFAULT NOW()
);