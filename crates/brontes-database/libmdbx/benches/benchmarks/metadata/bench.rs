use criterion::{criterion_group, BenchmarkId, Criterion};

const METADATA_QUERY: &str = "
SELECT
    block_number,
    block_hash,
    block_timestamp,
    relay_timestamp,
    p2p_timestamp,
    proposer_fee_recipient,
    proposer_mev_reward,
    mempool_flow
FROM brontes.metadata
WHERE block_number >= ? AND block_number < ? ";

criterion_group!(metadata,);
