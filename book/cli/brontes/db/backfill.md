# brontes db backfill

Identify data missing in libmdbx and backfill it

```bash
$ brontes db backfill --help
Usage: brontes db backfill [OPTIONS] --start-block <START_BLOCK> --end-block <END_BLOCK> --table <TABLE>

Options:
  -s, --start-block <START_BLOCK>
          Start Block

  -e, --end-block <END_BLOCK>
          block to trace to

      --brontes-db-path <BRONTES_DB_PATH>
          path to the brontes libmdbx db

  -t, --table <TABLE>
          Table to backfill

  -m, --max-tasks <MAX_TASKS>
          Max tasks to run

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