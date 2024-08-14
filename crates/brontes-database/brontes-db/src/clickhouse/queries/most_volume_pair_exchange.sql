WITH ranked_symbols AS
    (
        SELECT
            month,
            upper(replaceAll(replaceAll(replaceAll(symbol, '/', ''), '-', ''), '_', '')) AS symbol,
            exchange,
            ROW_NUMBER() OVER (PARTITION BY symbol ORDER BY sum_volume DESC) AS rn
        FROM cex.trading_volume_by_month
        WHERE (month >= toStartOfMonth(toDateTime(? / 1000000) - toIntervalMonth(1))) AND (month <= toStartOfMonth(toDateTime(? / 1000000) - toIntervalMonth(1)))
    )
SELECT
    symbol,
    exchange,
    toUnixTimestamp(month) * 1000000 AS timestamp
FROM ranked_symbols
WHERE rn = 1
ORDER BY timestamp
