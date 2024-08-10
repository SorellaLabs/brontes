with ranked_symbols as (
  select 
    month,
    symbol,
    exchange,
    ROW_NUMBER() OVER (PARTITION BY symbol ORDER BY sum_volume DESC) as rn
    from cex.trading_volume_by_month
    where month >= toStartOfMonth(toDateTime(? / 1000000) - INTERVAL 1 MONTH) and month <= toStartOfMonth(toDateTime(? / 1000000) - INTERVAL 1 MONTH)
)
SELECT symbol, exchange, toUnixTimestamp(month) * 1000000 as timestamp
FROM ranked_symbols
WHERE rn = 1
ORDER BY timestamp

