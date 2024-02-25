SELECT
    block_number,
    data
FROM brontes_api.tx_traces 
WHERE block_number >= ? AND block_number < ?