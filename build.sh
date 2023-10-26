#!/bin/sh

ERROR_FILE="build_error.txt"
COMMAND="cargo build --release --features server,test_run 2>> $ERROR_FILE"


errors() {
  echo "seeing if we need to rerun"

  OUT_FILE="failed_abis.txt"
  COUNT=$(grep -o -c "sol! (Contract0x[A-Fa-f0-9]*," "$1")
  echo "We have $COUNT"

  if [ "$COUNT" -eq 0 ];
  then
    echo "all contracts build, we are chilling"
  else
    grep -o "sol! (Contract0x[A-Fa-f0-9]*,"  $1 | grep -o "0x[A-Fa-f0-9]*," | sort -u >> "$OUT_FILE" | cat "$OUT_FILE" | sort -u > "$OUT_FILE"
    echo "have written the $COUNT failed contracts to $OUT_FILE, Now running $2"
    eval $2
  fi
}

# run inital command
echo "Running $COMMAND"
eval $COMMAND

# parse error file for sol! macro errors. if there is an error the bad addresses
# will be written to 
echo "$COMMAND finished running. Checking to see if we had failures"
errors "$ERROR_FILE" "$COMMAND"

