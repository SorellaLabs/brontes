-- Define composite types
CREATE TYPE contract_info_type AS (
    is_contract BOOLEAN,
    contract_type VARCHAR(255),
    decimals SMALLINT
);

CREATE TYPE socials_type AS (
    twitter VARCHAR(255),
    twitter_followers BIGINT,
    telegram VARCHAR(255),
    github VARCHAR(255),
    url VARCHAR(255)
);

-- Create the table
CREATE TABLE brontes_api.address_meta (
    address CHAR(66) NOT NULL,
    entity_name VARCHAR(255),
    nametag VARCHAR(255),
    labels VARCHAR(255)[] NOT NULL,
    type VARCHAR(255),
    contract_info contract_info_type,
    ens VARCHAR(255),
    socials socials_type,
    last_updated TIMESTAMP NOT NULL DEFAULT NOW()
);