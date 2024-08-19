# brontes db

Brontes database commands

```bash
$ brontes db --help
Usage: brontes db [OPTIONS] <COMMAND>

Commands:
  insert               Insert into the brontes libmdbx db
  query                Query data from any libmdbx table and pretty print it in stdout
  clear                Clear a libmdbx table
  generate-traces      Generates traces and store them in libmdbx (also clickhouse if --feature local-clickhouse)
  cex-query            Fetches Cex data from the Sorella DB
  init                 Fetch data from the api and insert it into libmdbx
  table-stats          Libmbdx Table Stats
  export               Export libmbdx data to parquet
  download-snapshot    Downloads a database snapshot. Without specified blocks, it fetches the full range. With start/end blocks, it downloads that range and merges it into the current database
  download-clickhouse  Downloads the db data from clickhouse
  r2-upload            For internal use only. Uploads snapshots of db every 100k blocks to r2
  test-traces-init     Traces all blocks required to run the tests and inserts them into clickhouse
  trace-at-tip         Generates traces up to chain tip and inserts them into libmbx
  run-discovery        Only runs discovery and inserts discovered protocols into clickhouse
  backfill             Identify data missing in libmdbx and backfill it
  help                 Print this message or the help of the given subcommand(s)

Options:
      --brontes-db-path <BRONTES_DB_PATH>
          path to the brontes libmdbx db

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version

Display:
  -v, --verbosity...
          Set the minimum log level.
          
          -v      Errors
          -vv     Warnings
          -vvv    Info
          -vvvv   Debug
          -vvvvv  Traces (warning: very verbose!)

      --quiet
          Silence all log output
```