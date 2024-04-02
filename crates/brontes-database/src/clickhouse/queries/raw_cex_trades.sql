SELECT
    exchange,
    upper(replaceAll(replaceAll(replaceAll(symbol, '/', ''), '-', ''), '_', '')) AS symbol,
    timestamp,
    side,
    price,
    amount
FROM cex.normalized_trades 
WHERE timestamp >= ? AND timestamp < ?
