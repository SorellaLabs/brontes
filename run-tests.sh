#!/bin/sh

setup() {
  rustup default nightly
  echo "setting up db at /home/data/brontes-ci/$1"
  mkdir -p "/home/data/brontes-ci/$1"

  cp /home/brontes-ci/.env .
  echo "BRONTES_DB_PATH=/home/data/brontes-ci/$1" >> .env 
  echo "BRONTES_TST_DB_PATH=/home/data/brontes-ci/$1" >> .env 
  
}

# deletes repo and test db
teardown() {
  echo "deleting db /home/data/brontes-ci/$1"
  # delete db
  rm -rf "/home/data/brontes-ci/$1"
  echo "deleting folder /home/brontes-ci/$1"
  # delete folder
  rm -rf "/home/brontes-ci/$1"
}

setup $1

IT="it";
TEST="test";
BENCH="bench";

# we put these in different folders so that if you're on a branch and change these, they will run the branch version
if [ "$2" = "$IT" ]; then 
  out= source /it.sh
fi 

if [ "$2" = "$TEST" ]; then 
  out=./test.sh
fi

if [ "$2" = "$BENCH" ]; then 
  out= ./bench.sh
fi 

teardown $1

if $out; then : ;else exit; fi

