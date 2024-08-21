WITH ranked_symbols AS
    (
        SELECT
            month,
            upper(replaceAll(replaceAll(replaceAll(symbol, '/', ''), '-', ''), '_', '')) AS symbol,
            exchange,
            sum(sum_volume) AS total_volume
        FROM cex.trading_volume_by_month
        WHERE (month >= toStartOfMonth(toDateTime(? / 1000000) - toIntervalMonth(1))) AND (month <= toStartOfMonth(toDateTime(? / 1000000) + toIntervalMonth(1)))
        GROUP BY month, symbol, exchange
    ),
    aggregated_exchanges AS
    (
        SELECT
            symbol,
            month,
            groupArray((exchange, total_volume)) AS exchanges_volumes
        FROM ranked_symbols
        GROUP BY symbol, month
    )
SELECT
    symbol,
    arrayMap(x -> x.1, arraySort(v -> -v.2, exchanges_volumes)) AS exchanges, 
    toUnixTimestamp(month) * 1000000 AS timestamp
FROM aggregated_exchanges
ORDER BY timestamp

