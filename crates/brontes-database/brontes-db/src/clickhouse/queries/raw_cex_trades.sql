SELECT
    exchange,
    upper(replaceAll(replaceAll(replaceAll(symbol, '/', ''), '-', ''), '_', '')) AS symbol,
    timestamp,
    side,
    if (side == 'sell', price, divide(1, price)) AS price,
    if (side == 'sell', amount, multiply(divide(1, price), amount)) AS amount
FROM cex.normalized_trades 
WHERE timestamp >= ? AND timestamp < ?
