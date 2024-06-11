  SELECT
      exchange,
      'Taker' as trade_type,
      upper(replaceAll(replaceAll(replaceAll(symbol, '/', ''), '-', ''), '_', '')) AS symbol,
      timestamp,
      side,
      if (side == 'sell', price, divide(1, price)) AS price,
      if (side == 'sell', amount, multiply(divide(1, price), amount)) AS amount
  FROM cex.normalized_trades 
  where timestamp >= ? AND timestamp < ?

  UNION ALL

  SELECT
      exchange,
      'Maker' as trade_type,
      upper(replaceAll(replaceAll(replaceAll(symbol, '/', ''), '-', ''), '_', '')) AS symbol,
      timestamp,
      if(side == 'sell', 'buy', 'sell') as side,
      if (side == 'sell', price, divide(1, price)) AS price,
      if (side == 'sell', amount, multiply(divide(1, price), amount)) AS amount
  FROM cex.normalized_trades 
  where timestamp >= ? AND timestamp < ?
