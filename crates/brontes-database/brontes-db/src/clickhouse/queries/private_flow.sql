SELECT
    tx_hash
FROM ethereum.`chainbound.mempool`
WHERE tx_hash IN ?
GROUP BY tx_hash