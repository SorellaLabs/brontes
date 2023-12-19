SELECT
    block_number,
    block_hash,
    relay_timestamp,
    p2p_timestamp,
    proposer_fee_recipient,
    proposer_mev_reward,
    CASE 
        WHEN has(mempool_flow, '') THEN []
        ELSE mempool_flow
    END
FROM brontes.metadata WHERE block_number >= ? AND block_number <= ?
