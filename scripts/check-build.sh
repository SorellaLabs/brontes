#!/bin/bash

# Stop on first error
set -e

echo "Running cargo build..."
cargo build --workspace --all-features

echo "Checking code format with cargo fmt..."
cargo fmt --all --check

echo "Running cargo clippy..."
cargo clippy --workspace --lib --examples --tests --benches --all-features -- -D warnings

echo "All build & format checks pass!"
