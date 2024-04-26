# brontes db query

Query data from any libmdbx table and pretty print it in stdout

```bash
$ brontes db query --help
Usage: brontes db query [OPTIONS] --table <TABLE> --key <KEY>

Options:
  -t, --table <TABLE>
          that table to query

  -k, --key <KEY>
          the key of the table being queried. if a range is wanted use the rust syntax of .. --key 80 or --key 80..100

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