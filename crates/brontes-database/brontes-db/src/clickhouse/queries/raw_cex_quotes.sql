SELECT
    c.exchange as exchange,
    upper(replaceAll(replaceAll(replaceAll(c.symbol, '/', ''), '-', ''), '_', '')) AS symbol,
    c.timestamp as timestamp,
    c.ask_amount as ask_amount,
    c.ask_price as ask_price,
    c.bid_price as bid_price,
    c.bid_amount as bid_amount
FROM cex.normalized_quotes as c
WHERE c.timestamp >= ? AND c.timestamp < ?