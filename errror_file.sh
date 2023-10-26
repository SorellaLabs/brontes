#!/bin/sh

echo "seeing if we need to rerun"

OUT_FILE="failed_abis.txt"
COUNT=$(grep -o -c "sol! (Contract0x[A-Fa-f0-9]*," "$1")
echo "We have $COUNT"

if [ "$COUNT" -eq 0 ];
then
  echo "all contracts build, we are chilling"
else
  grep -o "sol! (Contract0x[A-Fa-f0-9]*,"  $1 | grep -o "0x[A-Fa-f0-9]*," | sort -u >> "$OUT_FILE" | cat "$OUT_FILE" > "$OUT_FILE"
  echo "have written the $COUNT failed contracts to $OUT_FILE, Now running $2"
  $2
fi
