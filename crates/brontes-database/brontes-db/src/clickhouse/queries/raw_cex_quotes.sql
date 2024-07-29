SELECT
    c.exchange,
    upper(replaceAll(replaceAll(replaceAll(c.symbol, '/', ''), '-', ''), '_', '')) AS symbol,
    c.timestamp,
    c.ask_amount,
    c.ask_price,
    c.bid_price,
    c.bid_amount
FROM cex.normalized_quotes as c
WHERE c.timestamp >= ? AND c.timestamp < ?
