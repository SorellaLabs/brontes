WITH
    ? AS relay_time,
    ? AS p2p_time,
    prices AS (
        SELECT 
            substring(toString(sub1.address), 3) AS address,
            sub1.price AS relay_price,
            sub2.price AS p2p_price
        FROM
        (
            SELECT 
                max(bt.timestamp) as timestamp, 
                et.address as address, 
                round(avg(bt.ask_price + bt.bid_price)/2, 6) as price
            FROM 
                cex.normalized_quotes as bt
            INNER JOIN ethereum.tokens AS et 
            ON et.symbol = substring(bt.symbol, 1, length(bt.symbol) - 4)
            WHERE 
                (
                    (bt.timestamp <= relay_time) AND (bt.timestamp > relay_time - 1000000)
                )
                AND substring(bt.symbol, -4) = 'USDT'
            GROUP BY 
                address
        ) as sub1 INNER JOIN (
            SELECT 
                max(bt.timestamp) as timestamp, 
                et.address as address, 
                round(avg(bt.ask_price + bt.bid_price)/2, 6) as price
            FROM 
                cex.normalized_quotes as bt
            INNER JOIN ethereum.tokens AS et 
            ON et.symbol = substring(bt.symbol, 1, length(bt.symbol) - 4)
            WHERE 
                (
                    (bt.timestamp <= p2p_time) AND (bt.timestamp > p2p_time - 1000000)
                )
                AND substring(bt.symbol, -4) = 'USDT'
            GROUP BY 
                address
        ) AS sub2 ON sub2.address = sub1.address
    ),
    grouped_prices AS (
        SELECT 
            groupArray((address, (relay_price, p2p_price))) as grouped_prices
        FROM prices
    )
SELECT 
    grouped_prices.grouped_prices as token_prices
FROM grouped_prices
SETTINGS distributed_product_mode = 'allow'