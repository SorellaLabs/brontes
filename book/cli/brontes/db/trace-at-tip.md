# brontes db trace-at-tip

Generates traces up to chain tip and inserts them into libmbx

```bash
$ brontes db trace-at-tip --help
Usage: brontes db trace-at-tip [OPTIONS]

Options:
  -s, --start-block <START_BLOCK>
          Start Block

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