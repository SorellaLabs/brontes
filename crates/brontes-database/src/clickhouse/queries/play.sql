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
            (bt.timestamp <= 1699116960909) AND (bt.timestamp > 1699116960909 - 100000)
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
            (bt.timestamp <= 1699116962163) AND (bt.timestamp > 1699116962163 - 100000)
        )
        AND substring(bt.symbol, -4) = 'USDT'
    GROUP BY 
        address
) AS sub2 ON sub2.address = sub1.address




WITH
    1699116962163 AS p2p_time,
    bids_asks AS
    (
        SELECT
            max(bt.timestamp) AS timestamp,
            bt.symbol AS symbol,
            bt.exchange AS exchange,
            any(bt.ask_price) AS ask_price,
            any(bt.bid_price) AS bid_price
        FROM cex.normalized_quotes AS bt
        WHERE (bt.timestamp <= p2p_time) AND (bt.timestamp > (p2p_time - 1000000))
        GROUP BY
            bt.symbol,
            bt.exchange
    ),
    prices_a AS
    (
        SELECT
            ba.timestamp AS timestamp,
            ba.symbol AS pair,
            s.base_asset AS base_asset,
            s.quote_asset AS quote_asset,
            et1.address AS base_address,
            et2.address AS quote_address,
            ba.ask_price AS ask_price,
            ba.bid_price AS bid_price
        FROM bids_asks AS ba
        INNER JOIN cex.symbols AS s ON (s.pair = ba.symbol) AND (ba.exchange = s.exchange)
        INNER JOIN ethereum.tokens AS et2 ON et2.symbol = s.quote_asset
        INNER JOIN ethereum.tokens AS et1 ON et1.symbol = s.base_asset
    ),
    prices_b AS
    (
        SELECT
            ba.timestamp AS timestamp,
            concat(s.quote_asset, s.base_asset) AS pair,
            s.base_asset AS base_asset,
            s.quote_asset AS quote_asset,
            et2.address AS base_address,
            et1.address AS quote_address,
            1 / ba.ask_price AS bid_price,
            1 / ba.bid_price AS ask_price
        FROM bids_asks AS ba
        INNER JOIN cex.symbols AS s ON (s.pair = ba.symbol) AND (ba.exchange = s.exchange)
        INNER JOIN ethereum.tokens AS et2 ON et2.symbol = s.quote_asset
        INNER JOIN ethereum.tokens AS et1 ON et1.symbol = s.base_asset
    ),
    prices AS
    (
        SELECT (base_address, quote_address) as key, (timestamp, ask_price, bid_price) as val FROM (
            SELECT *
            FROM prices_a
            UNION ALL
            SELECT *
            FROM prices_b
        )
    )
SELECT *
FROM prices






SELECT 
    max(bt.timestamp) as timestamp,
    bt.symbol as symbol,
    any(bt.ask_price) as ask_price,
    any(bt.bid_price) as bid_price
FROM 
    cex.normalized_quotes as bt
WHERE bt.timestamp <= 1699116962163 AND (bt.timestamp > 1699116962163 - 1000000)
GROUP BY bt.symbol



SELECT 
    max(bt.timestamp) as timestamp, 
    any(s.base_asset) as base_symbol,
    any(s.quote_asset) as quote_asset,
    any(et.address) as base_address,
    any(bt.ask_price) as ask_price,
    any(bt.bid_price) as bid_price
FROM 
    cex.normalized_quotes as bt
INNER JOIN cex.symbols as s ON s.pair = bt.symbol AND bt.exchange = s.exchange
INNER JOIN ethereum.tokens AS et ON et.symbol = s.base_asset OR et.symbol = s.quote_asset
WHERE bt.timestamp <= 1699116962163 AND (bt.timestamp > 1699116962163 - 1000000)
GROUP BY bt.symbol





SELECT distinct(s.symbol)
FROM (
    SELECT base_asset  as symbol 
    FROM cex.symbols

    UNION ALL 

    SELECT quote_asset  as symbol 
    FROM cex.symbols
) s
LEFT JOIN ethereum.tokens t ON t.symbol = s.symbol
WHERE t.symbol IS NULL OR t.symbol = ''



SELECT max(bt.timestamp)
FROM 
    cex.binance_idv_symbol_tickers as bt
WHERE bt.timestamp <= 1699116962163
AND bt.symbol = 'ETHUSDT'