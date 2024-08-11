SELECT
    c.exchange AS exchange,
    'Taker' AS trade_type,
    upper(replaceAll(replaceAll(replaceAll(c.symbol, '/', ''), '-', ''), '_', '')) AS symbol,
    c.timestamp AS timestamp,
    c.side AS side,
    c.price AS price,
    c.amount AS amount
FROM cex.normalized_trades AS c 
WHERE c.timestamp >= ? AND c.timestamp < ?
