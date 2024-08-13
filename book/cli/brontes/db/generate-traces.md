# brontes db generate-traces

Generates traces and store them in libmdbx (also clickhouse if --feature local-clickhouse)

```bash
$ brontes db generate-traces --help
Usage: brontes db generate-traces [OPTIONS] --start-block <START_BLOCK> --end-block <END_BLOCK>

Options:
  -s, --start-block <START_BLOCK>
          Start Block

  -e, --end-block <END_BLOCK>
          block to trace to

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