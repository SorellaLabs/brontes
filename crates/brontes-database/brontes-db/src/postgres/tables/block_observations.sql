CREATE TABLE ethereum.block_observations (
    timestamp TIMESTAMP NOT NULL,
    block_number BIGINT NOT NULL,
    block_hash Hash256 NOT NULL,
    node_id VARCHAR(255) NOT NULL,
    source TEXT NOT NULL,
    vpc BOOLEAN NOT NULL
);
