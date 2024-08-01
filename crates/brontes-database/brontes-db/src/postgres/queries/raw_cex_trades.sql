SELECT
    c.exchange AS exchange,
    'Taker' AS trade_type,
    UPPER(REPLACE(REPLACE(REPLACE(c.symbol, '/', ''), '-', ''), '_', '')) AS symbol,
    c.timestamp AS timestamp,
    c.side AS side,
    c.price AS price,
    c.amount AS amount
FROM cex.normalized_trades AS c 
WHERE c.timestamp >= $1 AND c.timestamp < $2