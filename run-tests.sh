#!/bin/sh

setup() {
  if rustup default nightly; then : ;else return false; fi
  git checkout $1
  echo "setting up db at /home/data/brontes-ci/$2"
  mkdir -p "/home/data/brontes-ci/$2"

  if cp /home/brontes-ci/.env .; then :;else return false ;fi
  echo "BRONTES_DB_PATH=/home/data/brontes-ci/$2" >> .env 
  echo "BRONTES_TST_DB_PATH=/home/data/brontes-ci/$2" >> .env 
  
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

if setup $1 $2; then :;else teardown $2 ; exit; fi

IT="it";
TEST="test";
BENCH="bench";

# we put these in different folders so that if you're on a branch and change these, they will run the branch version
if [ "$3" = "$IT" ]; then 
  out=`./it.sh`
fi 

if [ "$3" = "$TEST" ]; then 
  out=`./test.sh`
fi

if [ "$3" = "$BENCH" ]; then 
  out=`./bench.sh`
fi 

teardown $2

if $out; then : ;else exit; fi

