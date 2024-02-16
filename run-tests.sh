#!/bin/sh
if ./it.sh $1; then : ; else exit; fi
if ./test.sh $1; then : ; else exit; fi
if ./bench.sh $1; then : ; else exit; fi
