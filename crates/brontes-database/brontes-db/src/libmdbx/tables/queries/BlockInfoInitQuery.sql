WITH
    ? AS start_block,
    ? AS end_block,
    relay_bids AS (
        SELECT
            block_number,
            if(ultrasound_adj_block_hash IS NOT NULL, ultrasound_adj_block_hash, block_hash) AS final_block_hash,
            anyLast(timestamp) AS relay_timestamp,
            anyLast(proposer_fee_recipient) AS proposer_fee_recipient,
            anyLast(if(ultrasound_adj_value IS NOT NULL, ultrasound_adj_value, value)) AS proposer_mev_reward
        FROM ethereum.relays 
        WHERE block_number >= start_block AND block_number < end_block AND value != 0 AND relays.proposer_fee_recipient != ''
        GROUP BY block_number, final_block_hash
    ),
    relay_payloads AS (
        SELECT
            block_number,
            block_hash,
            anyLast(proposer_fee_recipient) AS proposer_fee_recipient,
            anyLast(value) AS proposer_mev_reward
        FROM ethereum.relay_payloads
        WHERE block_number >= start_block AND block_number < end_block AND value != 0 AND relay_payloads.proposer_fee_recipient != ''
        GROUP BY block_number, block_hash
    ),
    raw_blocks AS (
        SELECT
            block_number,
            block_hash,
            anyLast(block_timestamp) AS block_timestamp
        FROM ethereum.blocks
        WHERE block_number >= start_block AND block_number < end_block AND valid = 1
        GROUP BY block_number, block_hash
    ),
    block_observations AS (
        SELECT
            block_number,
            block_hash,
            round(max(timestamp) / 1000) AS p2p_timestamp
        FROM ethereum.block_observations
        WHERE block_number >= start_block AND block_number < end_block
        GROUP BY block_number, block_hash
    ),
    private_txs AS (
        SELECT
            block_number,
            groupUniqArray(tx_hash) AS private_flow
        FROM eth_analytics.private_txs
        WHERE block_number >= start_block AND block_number < end_block
        GROUP BY block_number
    )
SELECT
    CAST(b.block_number, 'UInt64') AS block_number,
    CAST(b.block_hash, 'String') AS block_hash,
    CAST(b.block_timestamp, 'UInt64') AS block_timestamp,
    CAST(r.relay_timestamp, 'Nullable(UInt64)') AS relay_timestamp,
    CAST(o.p2p_timestamp, 'Nullable(UInt64)') AS p2p_timestamp,
    CAST(ifNull(r.proposer_fee_recipient, p.proposer_fee_recipient), 'Nullable(String)') AS proposer_fee_recipient,
    CAST(ifNull(r.proposer_mev_reward, p.proposer_mev_reward), 'Nullable(UInt128)') AS proposer_mev_reward,
    CAST(ifNull(v.private_flow, []), 'Array(String)') AS private_flow
FROM raw_blocks b
LEFT JOIN relay_bids r ON b.block_number = r.block_number AND b.block_hash = r.final_block_hash
LEFT JOIN relay_payloads p ON b.block_number = p.block_number AND b.block_hash = p.block_hash
LEFT JOIN block_observations o ON b.block_number = o.block_number AND b.block_hash = o.block_hash
LEFT JOIN private_txs v ON b.block_number = v.block_number
