WITH
    get_block_number AS (SELECT $1::BIGINT AS block_num),
    relay_bids AS (
        SELECT
            block_number,
            block_hash,
            MAX(timestamp) AS relay_timestamp,
            MAX(proposer_fee_recipient) AS proposer_fee_recipient,
            MAX(value) AS proposer_mev_reward
        FROM ethereum.relays 
        WHERE block_number = (SELECT block_num FROM get_block_number) AND value != 0 AND relays.proposer_fee_recipient != ''
        GROUP BY block_number, block_hash
    ),
    relay_payloads AS (
        SELECT
            block_number,
            block_hash,
            MAX(proposer_fee_recipient) AS proposer_fee_recipient,
            MAX(value) AS proposer_mev_reward
        FROM ethereum.relay_payloads
        WHERE block_number = (SELECT block_num FROM get_block_number) AND value != 0 AND relay_payloads.proposer_fee_recipient != ''
        GROUP BY block_number, block_hash
    ),
    raw_blocks AS (
        SELECT
            block_number,
            block_hash,
            MAX(block_timestamp) AS block_timestamp
        FROM ethereum.blocks
        WHERE block_number = (SELECT block_num FROM get_block_number) AND valid = 1
        GROUP BY block_number, block_hash
    ),
    block_observations AS (
        SELECT
            block_number,
            block_hash,
            ROUND(MAX(timestamp) / 1000) AS p2p_timestamp
        FROM ethereum.block_observations
        WHERE block_number = (SELECT block_num FROM get_block_number)
        GROUP BY block_number, block_hash
    ),
    private_txs AS (
        SELECT
            block_number,
            ARRAY_AGG(DISTINCT tx_hash) AS private_flow
        FROM eth_analytics.private_txs
        WHERE block_number = (SELECT block_num FROM get_block_number)
        GROUP BY block_number
    )
SELECT
    b.block_number::BIGINT AS block_number,
    b.block_hash::TEXT AS block_hash,
    b.block_timestamp AS block_timestamp,
    r.relay_timestamp AS relay_timestamp,
    o.p2p_timestamp AS p2p_timestamp,
    COALESCE(r.proposer_fee_recipient, p.proposer_fee_recipient)::TEXT AS proposer_fee_recipient,
    COALESCE(r.proposer_mev_reward, p.proposer_mev_reward)::NUMERIC AS proposer_mev_reward,
    COALESCE(v.private_flow, ARRAY[]::Hash256[])::Hash256[] AS private_flow
FROM raw_blocks b
LEFT JOIN relay_bids r ON b.block_number = r.block_number AND b.block_hash = r.block_hash
LEFT JOIN relay_payloads p ON b.block_number = p.block_number AND b.block_hash = p.block_hash
LEFT JOIN block_observations o ON b.block_number = o.block_number AND b.block_hash = o.block_hash
LEFT JOIN private_txs v ON b.block_number = v.block_number
