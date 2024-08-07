CREATE TABLE ethereum.pools (
    protocol VARCHAR(64) NOT NULL,
    protocol_subtype VARCHAR(64) NOT NULL,
    address CHAR(42) NOT NULL,
    tokens CHAR(42)[] NOT NULL,
    curve_lp_token CHAR(42),
    init_block BIGINT NOT NULL,
    PRIMARY KEY (protocol, address)
);
