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
        WHERE cb.block_hash = relay_block.block_hash
    ),
    private_flow AS
    (
        SELECT groupArray(toString(a.tx_hash)) AS flow
        FROM
        (
            SELECT DISTINCT arrayJoin(transaction_hashes) AS tx_hash
            FROM ethereum.blocks
            WHERE (blocks.block_number = block_number) AND (valid = 1)
        ) AS a
        LEFT JOIN
        (
            SELECT a.tx_hash AS tx_hash
            FROM ethereum.unique_mempool AS um
            INNER JOIN
            (
                SELECT DISTINCT arrayJoin(transaction_hashes) AS tx_hash
                FROM ethereum.blocks
                WHERE (blocks.block_number = block_number) AND (valid = 1)
            ) AS a ON a.tx_hash = um.tx_hash
        ) AS pub ON a.tx_hash = pub.tx_hash
        WHERE (pub.tx_hash = '') OR (pub.tx_hash IS NULL)
    )
SELECT 
    block_number,
    toString(relay_p2p.block_hash) AS block_hash,
    relay_p2p.relay_time AS relay_time,
    relay_p2p.p2p_time AS p2p_time,
    toString(relay_p2p.proposer_addr) AS proposer_addr,
    relay_p2p.proposer_reward AS proposer_reward,
    private_flow.flow
FROM relay_p2p, private_flow



