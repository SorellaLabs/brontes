SELECT
    block_number,
    CAST(block_hash, 'String') AS block_hash,
    CAST(block_timestamp, 'UInt64') AS block_timestamp,
    CAST(relay_timestamp, ' Nullable(UInt64)') AS relay_timestamp,
    CAST(p2p_timestamp, ' Nullable(UInt64)') AS p2p_timestamp,
    CAST(proposer_fee_recipient, ' Nullable(String)') AS proposer_fee_recipient,
    CAST(proposer_mev_reward, ' Nullable(UInt128)') AS proposer_mev_reward,
    CAST(private_flow, 'Array(String)') AS private_flow
FROM brontes.block_info
WHERE block_info.block_hash IS NOT NULL AND block_number >= ? AND block_number < ? 
