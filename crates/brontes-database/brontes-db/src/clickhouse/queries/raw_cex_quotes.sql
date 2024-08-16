WITH
    grouped_time AS (
        SELECT
            c.exchange as exchange,
            upper(replaceAll(replaceAll(replaceAll(c.symbol, '/', ''), '-', ''), '_', '')) AS symbol,
            toUnixTimestamp(toDateTime(round(c.timestamp / 1000000), 'UTC')) * 1000000 AS timestamp_sec,
            argMin(c.timestamp, abs(CAST(c.timestamp, 'Int64') - CAST(timestamp_sec, 'Int64'))) as timestamp,
            argMin(c.ask_amount, abs(CAST(c.timestamp, 'Int64') - CAST(timestamp_sec, 'Int64'))) as ask_amount,
            argMin(c.ask_price, abs(CAST(c.timestamp, 'Int64') - CAST(timestamp_sec, 'Int64'))) as ask_price,
            argMin(c.bid_price, abs(CAST(c.timestamp, 'Int64') - CAST(timestamp_sec, 'Int64'))) as bid_price,
            argMin(c.bid_amount, abs(CAST(c.timestamp, 'Int64') - CAST(timestamp_sec, 'Int64'))) as bid_amount
        FROM cex.normalized_quotes as c
        WHERE c.timestamp >= ? AND c.timestamp < ?
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
ORDER BY timestamp
