CREATE TABLE brontes_api.builder_info (
    address VARCHAR(255) NOT NULL,
    name VARCHAR(255),
    fund VARCHAR(255),
    pub_keys JSONB, -- Assuming this is a JSON array of public keys
    searchers_eoas JSONB, -- Assuming this is a JSON array of EOA addresses
    searchers_contracts JSONB, -- Assuming this is a JSON array of contract addresses
    ultrasound_relay_collateral_address VARCHAR(255),
    PRIMARY KEY (address)
);