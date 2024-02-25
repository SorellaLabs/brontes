SELECT 
    block_number,
    tx_idx,
    quote
FROM brontes_api.dex_price_mapping
WHERE block_number >= ? AND block_number < ?