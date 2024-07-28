WITH ranked_symbols AS (
  SELECT
    CASE 
      WHEN upper(replaceAll(replaceAll(replaceAll(c.symbol, '/', ''), '-', ''), '_', '')) LIKE '%USD' and c.exchange = 'coinbase'
      THEN replace(upper(replaceAll(replaceAll(replaceAll(c.symbol, '/', ''), '-', ''), '_', '')), 'USD', 'USDT') 
      ELSE upper(replaceAll(replaceAll(replaceAll(c.symbol, '/', ''), '-', ''), '_', '')) 
    END AS symbol,
      c.exchange as ex,
      sum(c.amount) as amount,
      ROW_NUMBER() OVER (PARTITION BY symbol ORDER BY amount DESC) as rn
  FROM cex.normalized_trades as c 
  where c.timestamp < ? and c.timestamp > ?
  group by symbol, ex
)
SELECT symbol, ex as exchange
FROM ranked_symbols
WHERE rn = 1
