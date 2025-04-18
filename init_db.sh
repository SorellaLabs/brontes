#!/bin/bash

USER="brontes"

echo "Creating Databases..."
clickhouse-client --query="
    CREATE DATABASE IF NOT EXISTS brontes;
    CREATE DATABASE IF NOT EXISTS brontes_api;
    CREATE DATABASE IF NOT EXISTS ethereum;
    CREATE DATABASE IF NOT EXISTS mev;
    CREATE DATABASE IF NOT EXISTS cex;
" --user $USER --password

echo "Initializing brontes tables..."
for sql_file in ./crates/brontes-database/brontes-db/src/clickhouse/tables/*.sql; do
    echo "Running $sql_file..."
    clickhouse-client --user $USER --multiquery < "$sql_file"
done
