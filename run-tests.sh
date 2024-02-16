#!/bin/sh
git pull
git checkout $1
git pull
rustup default nightly
# we put these in different folders so that if you're on a branch and change these, they will run the branch version
if ./it.sh $1; then : ; else exit; fi
if ./test.sh $1; then : ; else exit; fi
if ./bench.sh $1; then : ; else exit; fi
git checkout main
