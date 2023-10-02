SELECT
    max(relays.timestamp) AS relay_timestamp,
    toUInt64(round(max(cb.timestamp) / 1000)) AS p2p_timestamp,
    any(toString(relays.fee_recipient)) AS proposer_addr,
    any(relays.value) AS proposer_reward
FROM ethereum.relays 
INNER JOIN ethereum.block_observations as cb
ON ethereum.relays.block_number = cb.block_number
WHERE (block_number = ?) AND (block_hash = ?)

