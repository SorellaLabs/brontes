SELECT
    block_number,
    data
FROM brontes_api.cex_pricing 
WHERE block_number >= ? AND block_number < ?