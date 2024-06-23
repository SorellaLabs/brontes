# brontes db download-snapshot

downloads a db snapshot from the remote endpoint

```bash
$ brontes db download-snapshot --help
Usage: brontes db download-snapshot [OPTIONS] --write-location <WRITE_LOCATION>

Options:
  -e, --endpoint <ENDPOINT>
          endpoint url
          
          [default: https://pub-e19b2b40b9c14ec3836e65c2c04590ec.r2.dev]

  -w, --write-location <WRITE_LOCATION>
          where to write the database

      --brontes-db-path <BRONTES_DB_PATH>
          path to the brontes libmdbx db

      --overwrite-db
          overwrite the database if it already exists in the write location

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