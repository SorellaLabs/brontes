#!/bin/sh

setup() {
  mkdir -p /home/data/brontes-ci/$2
  cp /home/brontes-ci/.env .
  echo "BRONTES_DB_PATH=/home/data/brontes-ci/$2" >> .env 
  echo "BRONTES_TST_DB_PATH=/home/data/brontes-ci/$2" >> .env 
  
}

# deletes repo and test db
teardown() {
  # delete db
  rm -rf /home/data/brontes-ci/$2
  # delete folder
  rm -rf /home/brontes-ci/$2
}

setup()

git pull
git checkout $1
git pull
rustup default nightly
# we put these in different folders so that if you're on a branch and change these, they will run the branch version
if [ $3 -eq "it" ]; then 
  if ./it.sh $1; then : ; else exit; fi
fi 

if [ $3 -eq "test" ]; then 
  if ./test.sh $1; then : ; else exit; fi
fi

if [ $3 -eq "bench" ]; then 
  if ./bench.sh $1; then : ; else exit; fi
fi 

teardown()

