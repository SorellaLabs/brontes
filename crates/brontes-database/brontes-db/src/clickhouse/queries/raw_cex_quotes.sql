WITH
    ? AS start_time,
    ? AS end_time,
    grouped_time AS (
        SELECT
            c.exchange as exchange,
            upper(replaceAll(replaceAll(replaceAll(c.symbol, '/', ''), '-', ''), '_', '')) AS symbol,
            toDateTime(c.timestamp / 1000000, 'UTC') AS timestamp_sec,
            max(c.timestamp) as timestamp,
            argMax(c.ask_amount, c.timestamp) as ask_amount,
            argMax(c.ask_price, c.timestamp) as ask_price,
            argMax(c.bid_price, c.timestamp) as bid_price,
            argMax(c.bid_amount, c.timestamp) as bid_amount
        FROM cex.normalized_quotes as c
        WHERE c.timestamp >= start_time AND c.timestamp < end_time
        GROUP BY exchange, symbol, timestamp_sec
    )
SELECT
    exchange,
    symbol,
    timestamp,
    ask_amount,
    ask_price,
    bid_price,
    bid_amount
FROM grouped_time