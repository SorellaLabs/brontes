#!/bin/sh
git pull
git checkout $1
git pull
rustup default nightly
if cargo run --release --features sorella-server -- run --start-block 18300000 --end-block 18300002 --run-dex-pricing; then : ; else exit; fi
git checkout main
