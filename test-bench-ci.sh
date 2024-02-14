#!/bin/sh

run_tests() {
    git pull
    git checkout $1
    git pull
    rustup default nightly
    cargo +nightly test
    git checkout main
}

run_benchmarks() {
    rm -rf /home/data/brontes-test/*
    git pull
    git checkout $1
    git pull
    rustup default nightly
    cargo +nightly bench 
    git checkout main
}

run_tests
run_benchmarks

