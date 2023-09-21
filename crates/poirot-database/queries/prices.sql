
SELECT 
    sub1.address AS address,
    sub1.price AS relay_price,
    sub2.price AS p2p_price
FROM
(
    SELECT 
        any(bt.timestamp) as timestamp, 
        et.address as address, 
        avg(bt.price) as price
    FROM 
        cex.binance_trades as bt
    INNER JOIN ethereum.tokens AS et 
    ON et.symbol = substring(bt.symbol, 1, length(bt.symbol) - 4)
    WHERE 
        (
            (bt.timestamp < ? + 10000 AND bt.timestamp > ? - 10000)
        )
        AND substring(bt.symbol, -4) = 'USDT'
    GROUP BY 
        address
) as sub1 INNER JOIN (
    SELECT 
        any(bt.timestamp) as timestamp, 
        et.address as address, 
        avg(bt.price) as price
    FROM 
        cex.binance_trades as bt
    INNER JOIN ethereum.tokens AS et 
    ON et.symbol = substring(bt.symbol, 1, length(bt.symbol) - 4)
    WHERE 
        (
            (bt.timestamp < ? + 10000 AND bt.timestamp > ? - 10000) 
        )
        AND substring(bt.symbol, -4) = 'USDT'
    GROUP BY 
        address
) AS sub2 ON sub2.address = sub1.address

