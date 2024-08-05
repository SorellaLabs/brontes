SELECT
    block_number::BIGINT AS block_number,
    (block_timestamp * 1000000)::BIGINT AS block_timestamp
FROM ethereum.blocks
WHERE block_number >= $1 AND block_number < $2
