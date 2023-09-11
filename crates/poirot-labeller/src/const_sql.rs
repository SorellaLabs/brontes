pub const PRICE: &str = r#"SELECT any(timestamp) as timestamp, toString(exchange) as exchange, toString(symbol) as symbol, avg(tardis_trades.price) as price,
FROM tardis_trades
WHERE (timestamp < ? + 10000 AND timestamp > ? - 10000) OR (timestamp < ? + 10000 AND timestamp > ? - 10000)
GROUP BY exchange, symbol"#;

pub const PRIVATE_FLOW: &str = r#"SELECT tx_hash
FROM
(
    SELECT arrayJoin(transaction_hashes) AS tx_hash
    FROM blocks
    WHERE (block_number = ?) AND (block_hash = ?) AND (valid = 1)
) AS subquery
WHERE tx_hash NOT IN (
    SELECT tx_hash
    FROM unique_mempool
)"#;

pub const RELAYS_P2P_TIME: &str = r#"SELECT max(relays.timestamp) as relay_timestamp, max(cb.timestamp) as p2p_timestamp
FROM relays 
INNER JOIN chainbound_block_observations_remote as cb
ON relays.block_number = cb.block_number
WHERE  block_number = ? AND block_hash = ?"#;

