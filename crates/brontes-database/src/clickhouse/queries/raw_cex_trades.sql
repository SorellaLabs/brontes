SELECT
    exchange,
    upper(replaceAll(replaceAll(replaceAll(symbol, '/', ''), '-', ''), '_', '')) AS symbol,
    timestamp,
    side,
    if (side == 'buy', price, divide(1, price)) AS price,
    if (side == 'buy', amount, multiply(divide(1, price), amount)) AS amount
FROM cex.normalized_trades 
WHERE timestamp >= ? AND timestamp < ?






SELECT *
FROM cex.normalized_trades
WHERE exchange = 'okex'
AND timestamp >= (1701543815-6) * 1000000
AND timestamp < (1701543815+6) * 1000000