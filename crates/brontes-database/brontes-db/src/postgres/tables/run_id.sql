CREATE TABLE brontes.run_id (
    run_id BIGINT,
    last_updated TIMESTAMP DEFAULT NOW(),
    PRIMARY KEY (run_id)
);