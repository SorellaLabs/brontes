SELECT
    block_number,
    data AS meta
FROM brontes.cex_price_mapping
WHERE block_number >= ? AND block_number < ?
