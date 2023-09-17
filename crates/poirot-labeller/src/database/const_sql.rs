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
)"#;

pub const RELAY_P2P_TIMES: &str = r#"SELECT max(relays.timestamp) as relay_timestamp, max(cb.timestamp) as p2p_timestamp
FROM ethereum.relays 
INNER JOIN ethereum.chainbound_block_observations_remote as cb
ON ethereum.relays.block_number = cb.block_number
WHERE  block_number = ? AND block_hash = ?"#;

pub const PRICES: &str = r#"SELECT 
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
    bt.symbol;"#;

