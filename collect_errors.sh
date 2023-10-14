#!/bin/sh
MAX_BLOCK=18351854
OUR_START_BLOCK=$START_BLOCK
OUR_END_BLOCK=$START_BLOCK + 1

while [ OUR_END_BLOCK <= MAX_BLOCK ] 
do
  OUR_START_BLOCK = $OUR_START_BLOCK + 1
  OUR_END_BLOCK = $OUR_END_BLOCK + 1
  echo "Running block $OUR_START_BLOCK to $OUR_END_BLOCK"
  export START_BLOCK=$OUR_START_BLOCK
  export END_BLOCK=$OUR_END_BLOCK
  exec "cargo build --features test_run,server"
done 


