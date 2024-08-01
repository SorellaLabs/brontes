CREATE TABLE brontes.token_info (
    address Hash256 NOT NULL,
    symbol VARCHAR(20) NOT NULL,
    decimals SMALLINT NOT NULL,
    PRIMARY KEY (address)
);
