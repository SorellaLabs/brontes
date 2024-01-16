#![feature(trivial_bounds)]

use criterion::criterion_main;

mod benchmarks;
mod libmdbx_impl;
mod setup;

criterion_main! {
    benchmarks::metadata::bench::metadata,
}
