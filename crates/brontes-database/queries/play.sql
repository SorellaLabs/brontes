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
        cex.binance_idv_symbol_tickers as bt
    INNER JOIN ethereum.tokens AS et 
    ON et.symbol = substring(bt.symbol, 1, length(bt.symbol) - 4)
    WHERE 
        (
            (bt.timestamp <= 1687010592941) AND (bt.timestamp > 1687010592941 - 1000000)
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
        cex.binance_idv_symbol_tickers as bt
    INNER JOIN ethereum.tokens AS et 
    ON et.symbol = substring(bt.symbol, 1, length(bt.symbol) - 4)
    WHERE 
        (
            (bt.timestamp <= 1687010591000) AND (bt.timestamp > 1687010591000 - 1000000)
        )
        AND substring(bt.symbol, -4) = 'USDT'
    GROUP BY 
        address
) AS sub2 ON sub2.address = sub1.address



SELECT
    any(toString(block_hash)) AS block_hash,
    min(relays.timestamp) AS relay_time,
    toUInt64(round(max(cb.timestamp) / 1000)) AS p2p_time,
    any(toString(relays.proposer_fee_recipient)) AS proposer_addr,
    any(relays.value) AS proposer_reward
FROM ethereum.relays 
INNER JOIN ethereum.blocks AS blocks ON blocks.block_hash = relays.block_hash
INNER JOIN ethereum.block_observations AS cb
ON ethereum.relays.block_number = cb.block_number 
WHERE (relays.block_number = 17500000) AND blocks.valid = 1



SELECT
    max(bt.timestamp) AS timestamp,
    et.address AS address
FROM cex.binance_idv_symbol_tickers AS bt
INNER JOIN ethereum.tokens AS et ON et.symbol = substring(bt.symbol, 1, length(bt.symbol) - 4)
WHERE bt.timestamp <= 1687010591000 and bt.timestamp >= 1687010591000 - 1000 and substring(bt.symbol, -4) = 'USDT' 
GROUP BY address, (et.symbol, bt.symbol)

SELECT
    max(bt.timestamp) AS timestamp,
    et.address AS address
FROM cex.binance_idv_symbol_tickers AS bt
INNER JOIN ethereum.tokens AS et ON et.symbol = substring(bt.symbol, 1, length(bt.symbol) - 4) OR et.symbol = substring(bt.symbol, length(bt.symbol) - 4)
WHERE bt.timestamp <= 1687010591000 and bt.timestamp >= 1687010591000 - 1000 and substring(bt.symbol, -4) = 'USDT' 
GROUP BY address, (et.symbol, bt.symbol)


    SELECT 
        max(bt.timestamp) as timestamp,
        et.address as address, 
        round(avg(bt.ask_price + bt.bid_price)/2, 6) as price,
        (et.symbol, bt.symbol)
    FROM 
        cex.binance_idv_symbol_tickers as bt
    INNER JOIN ethereum.tokens AS et 
    ON et.symbol = substring(bt.symbol, 1, length(bt.symbol) - 4)
    WHERE 
        (
            (bt.timestamp <= 1683302569691)
        )
        AND bt.symbol '%USDT%'
    GROUP BY 
        address, (et.symbol, bt.symbol)


SELECT 
    max(bt.timestamp) as timestamp,
    et.address as address, 
    round(avg(bt.ask_price + bt.bid_price)/2, 6) as price,
    (et.symbol, bt.symbol)
FROM 
    cex.binance_idv_symbol_tickers as bt
INNER JOIN ethereum.tokens AS et 
ON lower(et.symbol) LIKE lower(bt.symbol)
WHERE 
    (
        (bt.timestamp <= 1683302569691)
    )
    AND bt.symbol '%USDT%'
GROUP BY 
    address, (et.symbol, bt.symbol)



SELECT 
    max(bt.timestamp) as timestamp,
    et.address as address, 
    round(avg((bt.ask_price + bt.bid_price) / 2), 6) as price,
    (et.symbol, bt.symbol)
FROM 
    cex.binance_idv_symbol_tickers as bt
LEFT JOIN ethereum.tokens AS et 
    ON et.symbol = substring(bt.symbol, 1, length(bt.symbol) - 4)
WHERE 
    bt.timestamp <= 1683302569691
    AND bt.symbol LIKE '%USDT%'
GROUP BY 
    address, (et.symbol, bt.symbol)


