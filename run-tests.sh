#!/bin/sh

setup() {
  echo "setting up db at /home/data/brontes-ci/$1"
  mkdir -p "/home/data/brontes-ci/$1"

  cp /home/brontes-ci/.env .
  echo "BRONTES_DB_PATH=/home/data/brontes-ci/$1" >> .env 
  echo "BRONTES_TST_DB_PATH=/home/data/brontes-ci/$1" >> .env 
  
}

# deletes repo and test db
teardown() {
  echo "deleting db"
  # delete db
  rm -rf "/home/data/brontes-ci/$1"
  echo "deleting folder
  # delete folder
  rm -rf "/home/brontes-ci/$1"
}

setup

rustup default nightly
# we put these in different folders so that if you're on a branch and change these, they will run the branch version
if [ "$2" -eq "it" ]; then 
  out=./it.sh
fi 

if [ "$2" -eq "test" ]; then 
  out=./test.sh
fi

if [ "$2" -eq "bench" ]; then 
  out=./bench.sh
fi 

teardown

if $out; then : ;else exit; fi

