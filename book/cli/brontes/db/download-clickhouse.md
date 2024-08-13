# brontes db download-clickhouse

Downloads the db data from clickhouse

```bash
$ brontes db download-clickhouse --help
Usage: brontes db download-clickhouse [OPTIONS] --start-block <START_BLOCK> --end-block <END_BLOCK> --table <TABLE>

Options:
  -s, --start-block <START_BLOCK>
          Start block

  -e, --end-block <END_BLOCK>
          End block

      --brontes-db-path <BRONTES_DB_PATH>
          path to the brontes libmdbx db

  -t, --table <TABLE>
          Table to download

  -c, --clear-table
          Clear the table before downloading

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