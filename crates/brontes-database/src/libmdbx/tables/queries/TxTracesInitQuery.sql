SELECT
    block_number,
    traces
FROM brontes_api.tx_traces
WHERE block_number >= ? AND block_number < ?
