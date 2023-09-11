SELECT any(timestamp) as timestamp, toString(exchange) as exchange, toString(symbol) as symbol, avg(tardis_trades.price) as price,
FROM tardis_trades
WHERE (timestamp < ? + 10000 AND timestamp > ? - 10000) OR (timestamp < ? + 10000 AND timestamp > ? - 10000)
GROUP BY exchange, symbol