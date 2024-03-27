SELECT
    block_number,
    groupArrayArray(data) as data
FROM brontes_api.cex_trades
WHERE block_number >= ? AND block_number < ?
GROUP BY block_number

