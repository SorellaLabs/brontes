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
            (bt.timestamp <= 1699387260058) AND (bt.timestamp > 1699387260058 - 1000000)
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
            (bt.timestamp <= 1699387261752) AND (bt.timestamp > 1699387261752 - 1000000)
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
WHERE (relays.block_number = 18522330) AND blocks.valid = 1