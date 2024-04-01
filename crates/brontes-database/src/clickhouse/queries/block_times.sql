SELECT
    block_number,
    block_timestamp * 1000 AS timestamp
FROM brontes_api.block_info
WHERE block_number >= ? AND block_number < ?
