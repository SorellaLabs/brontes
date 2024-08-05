CREATE TABLE brontes_api.tx_traces (
    block_number BIGINT,
    trace JSON,
    last_updated BIGINT DEFAULT EXTRACT(EPOCH FROM CURRENT_TIMESTAMP)
);