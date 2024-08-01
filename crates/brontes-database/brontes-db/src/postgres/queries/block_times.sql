SELECT
    block_number AS block_number,
    block_timestamp AS block_timestamp
FROM ethereum.blocks
WHERE block_number >= $1 AND block_number < $2
