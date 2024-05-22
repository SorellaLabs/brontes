SELECT 
    block_number,
    tx_idx,
    quote
FROM brontes.dex_price_mapping
WHERE block_number >= ? AND block_number < ?
