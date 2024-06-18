  SELECT
      c.exchange as exchange,
      'Taker' as trade_type,
      upper(replaceAll(replaceAll(replaceAll(c.symbol, '/', ''), '-', ''), '_', '')) AS symbol,
      c.timestamp as timestamp,
      c.side as side,
      if (side == 'buy', c.price, divide(1, c.price)) AS price,
      if (side == 'buy', c.amount, multiply(divide(1, c.price), c.amount)) AS amount
  FROM cex.normalized_trades as c 
  where c.timestamp >= ? AND c.timestamp < ?

  UNION ALL

  SELECT
      c.exchange as exchange,
      'Maker' as trade_type,
      upper(replaceAll(replaceAll(replaceAll(c.symbol, '/', ''), '-', ''), '_', '')) AS symbol,
      c.timestamp as timestamp,
      if(c.side == 'buy', 'sell', 'buy') as side,
      if (side == 'buy', c.price, divide(1, c.price)) AS price,
      if (side == 'buy', c.amount, multiply(divide(1, c.price), c.amount)) AS amount
  FROM cex.normalized_trades as c
  where c.timestamp >= ? AND c.timestamp < ?
