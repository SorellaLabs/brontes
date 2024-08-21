SELECT DISTINCT
    tx_hash
FROM ethereum.`chainbound.mempool`
WHERE tx_hash IN (
    SELECT arrayJoin(?) AS tx_hash
)