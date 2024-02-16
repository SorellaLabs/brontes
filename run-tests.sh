#!/bin/sh

setup() {
  mkdir -p "/home/data/brontes-ci/$1"
  cp /home/brontes-ci/.env .
  echo "BRONTES_DB_PATH=/home/data/brontes-ci/$1" >> .env 
  echo "BRONTES_TST_DB_PATH=/home/data/brontes-ci/$1" >> .env 
  
}

# deletes repo and test db
teardown() {
  # delete db
  rm -rf "/home/data/brontes-ci/$1"
  # delete folder
  rm -rf "/home/brontes-ci/$1"
}

setup

rustup default nightly
# we put these in different folders so that if you're on a branch and change these, they will run the branch version
if [ "$2" -eq "it" ]; then 
  if ./it.sh; then : ; else exit; fi
fi 

if [ "$2" -eq "test" ]; then 
  if ./test.sh; then : ; else exit; fi
fi

if [ "$2" -eq "bench" ]; then 
  if ./bench.sh; then : ; else exit; fi
fi 

teardown
