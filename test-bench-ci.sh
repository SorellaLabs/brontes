#!/bin/sh

run_tests() {
    git pull
    git checkout $1
    git pull
    rustup default nightly
    if cargo +nightly test; then : ; else exit; fi
    git checkout main
}

run_benchmarks() {
    rm -rf /home/data/brontes-test/*
    git pull
    git checkout $1
    git pull
    rustup default nightly
    if cargo +nightly bench; then : ; else exit; fi
    git checkout main
}

run_tests
run_benchmarks
