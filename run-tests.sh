#!/bin/sh

setup() {
  mkdir /home/data/brontes-ci/$2
  cp ../.env .env
}

# deletes repo and test db
teardown() {
  rm -rf /home/data/brontes-ci/$2
  cd ..
  rm -rf $2
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

