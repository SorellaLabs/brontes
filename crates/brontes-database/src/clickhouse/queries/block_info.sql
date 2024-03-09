SELECT
    block_number,
    block_hash,
    block_timestamp,
    relay_timestamp,
    p2p_timestamp,
    proposer_fee_recipient,
    proposer_mev_reward,
    private_flow
FROM brontes_api.block_info
WHERE block_info.block_hash IS NOT NULL AND block_number = ? 
