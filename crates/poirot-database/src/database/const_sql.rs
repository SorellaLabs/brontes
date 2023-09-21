pub const PRIVATE_FLOW: &str = r#"SELECT tx_hash
FROM
(
    SELECT arrayJoin(transaction_hashes) AS tx_hash
    FROM ethereum.blocks
    WHERE (block_number = ?) AND (block_hash = ?) AND (valid = 1)
) AS subquery
WHERE tx_hash NOT IN (
    SELECT tx_hash
    FROM ethereum.unique_mempool
)


"#;

pub const RELAY_P2P_TIMES: &str = r#"SELECT max(relays.timestamp) as relay_timestamp, max(cb.timestamp) as p2p_timestamp
FROM ethereum.relays 
INNER JOIN ethereum.chainbound_block_observations_remote as cb
ON ethereum.relays.block_number = cb.block_number
WHERE  block_number = ? AND block_hash = ?"#;

pub const PRICES: &str = r#"SELECT 
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
) AS sub2 ON sub2.address = sub1.address"#;

