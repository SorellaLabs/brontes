#!/bin/bash
# run tests for all different workspaces collecting missing blocks until there are no more errors 

MISSING=`cargo test | grep -o 'BlockTraceError([0-9]\{1,9\}' | cut -c17- | sed '$!s/$/,/' | tr -d '\n'`

if [ ${#MISSING} -ne 0 ]; then 
  echo "inserting missing blocks $MISSING"
  if cargo run --features sorella-server -- db test-traces-init --blocks $MISSING; then : ; else return 1; fi
fi
echo "done"

