CREATE TABLE brontes.run_id (
    run_id BIGINT,
    last_updated BIGINT DEFAULT EXTRACT(EPOCH FROM CURRENT_TIMESTAMP)
);

CREATE INDEX idx_run_id ON brontes.run_id (run_id);