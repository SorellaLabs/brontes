CREATE TABLE brontes_api.tx_traces (
    block_number BIGINT NOT NULL,
    tx_hash CHAR(66) NOT NULL,  -- Ethereum transaction hash in hex format (0x...)
    traces BLOB NOT NULL,       -- Serialized form of TxTracesInner struct, likely in binary format
    gas_used BIGINT UNSIGNED,
    effective_price BIGINT UNSIGNED,
    tx_index INT UNSIGNED,
    is_success BOOLEAN,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    PRIMARY KEY (block_number, tx_hash),
    INDEX (block_number)  -- Index for efficient range queries on block_number
)