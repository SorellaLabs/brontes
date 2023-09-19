SELECT sub1.symbol as symbol, sub1.price as price0, sub2.price as price1
FROM (
    SELECT 
    any(bt.timestamp) as timestamp, 
    substring(bt.symbol, 1, length(bt.symbol) - 4) as symbol, 
    avg(bt.price) as price
FROM 
    cex.binance_trades as bt
WHERE 
    (bt.timestamp < ? + 10000 AND bt.timestamp > ? - 10000) 
    AND substring(bt.symbol, -4) = 'USDT'
        GROUP BY 
    bt.symbol
) as sub1
INNER JOIN (
    SELECT 
    any(bt.timestamp) as timestamp, 
    substring(bt.symbol, 1, length(bt.symbol) - 4) as symbol, 
    avg(bt.price) as price
FROM 
    cex.binance_trades as bt
WHERE 
    (bt.timestamp < ? + 10000 AND bt.timestamp > ? - 10000) 
    AND substring(bt.symbol, -4) = 'USDT'
    GROUP BY 
    bt.symbol
) as sub2 
ON sub2.symbol = sub1.symbol

