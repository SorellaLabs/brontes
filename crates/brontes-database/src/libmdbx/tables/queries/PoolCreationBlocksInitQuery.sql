SELECT
    block_number,
    pools
FROM brontes.pool_creation_block WHERE block_number >= ? AND block_number <= ?
