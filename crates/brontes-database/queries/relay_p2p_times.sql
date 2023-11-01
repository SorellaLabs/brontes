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
WHERE (relays.block_number = ?) AND blocks.valid = 1