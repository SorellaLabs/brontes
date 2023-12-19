SELECT
    block_number,
    groupArray((pair, metadata)) AS meta
FROM brontes.cex_price_mapping
GROUP BY block_number HAVING block_number >= ? AND block_number <= ?
