#!/bin/sh

ERROR_FILE="build_error.txt"
COMMAND=$(cargo build --release --features server,test_run 2>> "$ERROR_FILE")

# run inital command
echo "Running $COMMAND"
$COMMAND

# parse error file for sol! macro errors. if there is an error the bad addresses
# will be written to 
echo "$COMMAND finished running. Checking to see if we had failures"
./error_file.sh "$ERROR_FILE" "$COMMAND"

