#!/bin/sh

setup() {
  if rustup default nightly; then : ;else return 1; fi
  git checkout $1
  echo "setting up db at /home/data/brontes-ci/$2"
  mkdir -p "/home/data/brontes-ci/$2"

  if cp /home/brontes-ci/.env .env; then :;else return 1;fi

  echo "BRONTES_DB_PATH='/home/data/brontes-ci/$2'" >> .env 
  echo "BRONTES_TEST_DB_PATH='/home/data/brontes-ci/$2'" >> .env 
  echo "updated .env"
  
}

# deletes repo and test db
teardown() {
  if [ ${#1} -eq 0 ]; then 
    echo "Invalid teardown, will delete config"
    exit 1;
  fi 

  echo "deleting db /home/data/brontes-ci/$1"
  # delete db
  rm -rf "/home/data/brontes-ci/$1"
  echo "deleting folder /home/brontes-ci/$1"
  # delete folder
  rm -rf "/home/brontes-ci/$1"
}

if setup $1 $2; then 
  :
else 
  teardown $2 
  exit 1
fi

IT="it";
TEST="test";
BENCH="bench";

# we put these in different folders so that if you're on a branch and change these, they will run the branch version
if [ "$3" = "$IT" ]; then 
  if cargo run --release --features $4 -- run --start-block 18300000 --end-block 18300002 --run-dex-pricing; then : ; else teardown $2; exit 1; fi
fi 

if [ "$3" = "$TEST" ]; then 
  if cargo +nightly test --features $4; then : ;else  teardown $2; exit 1; fi
fi

if [ "$3" = "$BENCH" ]; then 
  if cargo +nightly bench --features $4; then : ; else teardown $2; exit 1; fi
fi 

teardown $2

