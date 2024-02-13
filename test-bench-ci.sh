#!/bin/sh

run_tests() {
  exec git pull
  exec git checkout $1
  exec git pull
  exec rustup default nightly
  exec cargo +nightly test
  exec git checkout main
}

run_benchmarks() {
  exec rm -rf /home/data/brontes-test/*
  exec git pull
  exec git checkout $1
  exec git pull
  exec rustup default nightly
  exec cargo +nightly bench 
  exec git checkout main
}

# simple lock to ensure only one ci can be running at once
while [ $CI_RUNNING -eq "TRUE" ] 
do

done

export CI_RUNNING="TRUE"
run_tests()
run_benchmarks()
export CI_RUNNING="FALSE"


