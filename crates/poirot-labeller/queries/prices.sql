SELECT 
    any(bt.timestamp) as timestamp, 
    substring(bt.symbol, 1, length(bt.symbol) - 4) as symbol, 
    avg(bt.price) as price
FROM 
    cex.binance_trades as bt
WHERE 
    (
        (bt.timestamp < ? + 10000 AND bt.timestamp > ? - 10000) 
        OR 
        (bt.timestamp < ? + 10000 AND bt.timestamp > ? - 10000)
    )
    AND substring(bt.symbol, -4) = 'USDT'
GROUP BY 
    bt.symbol;