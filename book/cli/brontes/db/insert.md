# brontes db insert

Insert into the brontes libmdbx db

```bash
$ brontes db insert --help
Usage: brontes db insert [OPTIONS] --table <TABLE> --key <KEY> --value <VALUE>

Options:
  -t, --table <TABLE>
          Table to query

  -k, --key <KEY>
          Key to query

      --brontes-db-path <BRONTES_DB_PATH>
          path to the brontes libmdbx db

      --value <VALUE>
          Value to insert

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