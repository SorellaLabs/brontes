  SELECT
      c.exchange as exchange,
      'Taker' as trade_type,
      upper(replaceAll(replaceAll(replaceAll(c.symbol, '/', ''), '-', ''), '_', '')) AS symbol,
      c.timestamp as timestamp,
      c.side as side,
      c.price AS price,
      c.amount AS amount
  FROM cex.normalized_trades as c 
  where c.timestamp >= ? AND c.timestamp < ?