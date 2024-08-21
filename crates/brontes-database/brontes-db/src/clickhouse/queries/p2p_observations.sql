SELECT 
    CAST(round(min(timestamp) / 1000), 'UInt64') AS first_observation
FROM ethereum.`chainbound.block_observations`
WHERE block_number = ? AND block_hash = ?