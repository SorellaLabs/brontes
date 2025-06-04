#!/bin/bash

USER="brontes"
HOST="${HOST:-localhost}"

echo "Connecting to ClickHouse at $HOST as user $USER..."

# Get password securely
read -s -p "Enter ClickHouse password: " PASSWORD
echo

echo "Dropping existing databases..."
clickhouse client --query="
    DROP DATABASE IF EXISTS brontes;
    DROP DATABASE IF EXISTS brontes_api;
    DROP DATABASE IF EXISTS ethereum;
    DROP DATABASE IF EXISTS mev;
    DROP DATABASE IF EXISTS cex;
    DROP DATABASE IF EXISTS timeboost;
" --user $USER --password "$PASSWORD"

echo "Recreating databases..."
clickhouse client --query="
    CREATE DATABASE brontes;
    CREATE DATABASE brontes_api;
    CREATE DATABASE ethereum;
    CREATE DATABASE mev;
    CREATE DATABASE cex;
    CREATE DATABASE timeboost;
" --user $USER --password "$PASSWORD"

echo "Initializing brontes tables..."
for sql_file in ./crates/brontes-database/brontes-db/src/clickhouse/tables/*.sql; do
    echo "Running $sql_file..."
    clickhouse client --host $HOST --user $USER --multiquery --password "$PASSWORD" < "$sql_file"
done

echo "Database reset complete!"
