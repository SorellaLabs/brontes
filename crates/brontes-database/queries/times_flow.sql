WITH
    ? AS block_number,
    relay_block AS (
        SELECT
            block_number AS block_num,
            toString(blocks.block_hash) AS block_hash,
            relays.timestamp AS relay_time,
            toString(relays.proposer_fee_recipient) AS proposer_addr,
            relays.value AS proposer_reward
        FROM ethereum.relays
        INNER JOIN ethereum.blocks AS blocks ON blocks.block_number = block_number AND blocks.block_hash = ethereum.relays.block_hash AND blocks.valid = 1
        WHERE (relays.block_number = block_number) 
    ),
    relay_p2p AS (
        SELECT
            any(relay_block.block_hash) AS block_hash,
            any(relay_block.relay_time) AS relay_time,
            toUInt64(round(max(cb.timestamp) / 1000)) AS p2p_time,
            any(relay_block.proposer_addr) AS proposer_addr,
            any(relay_block.proposer_reward) AS proposer_reward
        FROM relay_block
        INNER JOIN ethereum.block_observations AS cb ON cb.block_number = relay_block.block_num
    ),
    private_flow AS (
        SELECT bt.tx_hash
        FROM
        (
            SELECT toString(arrayJoin(transaction_hashes)) AS tx_hash
            FROM ethereum.blocks
            WHERE (blocks.block_number = block_number) AND (valid = 1)
        ) AS bt
        WHERE bt.tx_hash NOT IN (
            SELECT DISTINCT um.tx_hash
            FROM ethereum.unique_mempool AS um
            WHERE um.tx_hash IN (
                SELECT toString(arrayJoin(transaction_hashes)) AS tx_hash
                FROM ethereum.blocks
                WHERE (blocks.block_number = block_number) AND (valid = 1)
            )
        )
    ),
    grouped_private_flow AS (
        SELECT 
            groupArray(tx_hash) AS private_flow
        FROM private_flow
    )
SELECT 
    block_number,
    relay_p2p.block_hash AS block_hash,
    relay_p2p.relay_time AS relay_time,
    relay_p2p.p2p_time AS p2p_time,
    relay_p2p.proposer_addr AS proposer_fee_recipient,
    relay_p2p.proposer_reward AS proposer_reward
    grouped_private_flow.private_flow
FROM relay_p2p, grouped_private_flow