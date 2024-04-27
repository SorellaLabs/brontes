# brontes db

Brontes database commands

```bash
$ brontes db --help
Usage: brontes db [OPTIONS] <COMMAND>

Commands:
  insert            Allows for inserting items into libmdbx
  query             Query data from any libmdbx table and pretty print it in stdout
  clear             Clear a libmdbx table
  generate-traces   Generates traces and will store them in libmdbx (also clickhouse if --feature local-clickhouse)
  libmdbx-mem-test  Test libmdbx memory usage
  init              For a given range, will fetch all data from the api and insert it into libmdbx
  export            Export libmbdx data to parquet
  test-traces-init  Traces all blocks needed for testing and inserts them into clickhouse
  help              Print this message or the help of the given subcommand(s)

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