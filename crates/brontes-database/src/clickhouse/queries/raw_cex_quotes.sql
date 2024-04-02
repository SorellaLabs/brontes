SELECT
    exchange,
    upper(replaceAll(replaceAll(replaceAll(symbol, '/', ''), '-', ''), '_', '')) AS symbol,
    timestamp,
    ask_amount,
    ask_price,
    bid_price,
    bid_amount
FROM cex.normalized_quotes 
WHERE timestamp >= ? AND timestamp < ?
