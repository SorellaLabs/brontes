# brontes db run-discovery

Only runs discovery and inserts discovered protocols into clickhouse

```bash
$ brontes db run-discovery --help
Usage: brontes db run-discovery [OPTIONS]

Options:
  -s, --start-block <START_BLOCK>
          Start Block

  -m, --max-tasks <MAX_TASKS>
          Max number of tasks to run concurrently

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