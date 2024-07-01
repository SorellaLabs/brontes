SELECT
    CAST(block_number, 'UInt64') AS block_number,
    CAST(block_timestamp * 1000000, 'UInt64') AS block_timestamp
FROM ethereum.blocks
WHERE block_number >= ? AND block_number < ?
