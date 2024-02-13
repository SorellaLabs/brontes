#!/bin/sh

write_lock() {
  echo $1 > .ci_lock
}
read_lock() {
  lock=`cat .ci_lock`
}

run_tests() {
  exec | 
    git pull
    git checkout $1
    git pull
    rustup default nightly
    cargo +nightly test
    git checkout main
}

run_benchmarks() {
  exec | 
    rm -rf /home/data/brontes-test/*
    git pull
    git checkout $1
    git pull
    rustup default nightly
    cargo +nightly bench 
    git checkout main
}

# simple lock to ensure only one ci can be running at once
read_lock
while [ $lock -eq 1 ]; do
  read_lock
done

write_lock 1
run_tests
run_benchmarks
write_lock 0

