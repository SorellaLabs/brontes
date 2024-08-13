# brontes db test-traces-init

Traces all blocks required to run the tests and inserts them into clickhouse

```bash
$ brontes db test-traces-init --help
Usage: brontes db test-traces-init [OPTIONS]

Options:
  -b, --blocks <BLOCKS>
          Blocks to trace

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