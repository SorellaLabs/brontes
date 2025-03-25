-- Create the address_meta table
CREATE TABLE IF NOT EXISTS brontes_api.address_meta (
    address BINARY(20) NOT NULL COMMENT 'Ethereum address in binary format',
    entity_name VARCHAR(255) COMMENT 'Name of the entity associated with this address',
    nametag VARCHAR(255) COMMENT 'Custom nametag for the address',
    labels TEXT COMMENT 'Labels or tags associated with this address',
    type VARCHAR(50) COMMENT 'Type of address (e.g., EOA, Contract, etc.)',
    contract_info TEXT COMMENT 'JSON containing contract information if applicable',
    ens VARCHAR(255) COMMENT 'Ethereum Name Service name',
    socials TEXT COMMENT 'JSON containing social media links and information',
    PRIMARY KEY (address)
) ENGINE=MergeTree()
ORDER BY address;

-- Optional indexes for better query performance
CREATE INDEX IF NOT EXISTS idx_entity_name ON brontes_api.address_meta (entity_name);
CREATE INDEX IF NOT EXISTS idx_type ON brontes_api.address_meta (type);
CREATE INDEX IF NOT EXISTS idx_ens ON brontes_api.address_meta (ens);