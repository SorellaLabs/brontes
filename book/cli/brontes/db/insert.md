# brontes db insert

Allows for inserting items into libmdbx

```bash
$ brontes db insert --help
Usage: brontes db insert [OPTIONS] --table <TABLE> --key <KEY> --value <VALUE>

Options:
  -t, --table <TABLE>
          that table to be queried

  -k, --key <KEY>
          

      --brontes-db-path <BRONTES_DB_PATH>
          path to the brontes libmdbx db

      --value <VALUE>
          

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