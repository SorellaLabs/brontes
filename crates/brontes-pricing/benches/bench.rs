mod it;
mod state;
use criterion::{criterion_group, criterion_main};

// criterion_group!(it_runs, it::bench_block_pricing);
criterion_group!(v3, state::bench_v3_price_requests, state::bench_v3_state_loads);
criterion_group!(v2, state::bench_v2_price_requests, state::bench_v2_state_loads);

criterion_main!( v2, v3);
