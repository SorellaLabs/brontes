SELECT
    m.block_number AS block_number,
    CAST(m.block_hash, 'String') AS block_hash,
    CAST(m.block_timestamp, 'UInt64') AS block_timestamp,
    CAST(m.relay_timestamp, ' Nullable(UInt64)') AS relay_timestamp,
    CAST(m.p2p_timestamp, ' Nullable(UInt64)') AS p2p_timestamp,
    CAST(m.proposer_fee_recipient, ' Nullable(String)') AS proposer_fee_recipient,
    CAST(m.proposer_mev_reward, ' Nullable(UInt128)') AS proposer_mev_reward,
    CAST(m.private_flow, 'Array(String)') AS private_flow
FROM brontes.metadata_util m
WHERE m.block_hash IS NOT NULL AND m.block_number >= ? AND m.block_number < ?
