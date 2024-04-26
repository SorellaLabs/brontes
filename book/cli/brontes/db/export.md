# brontes db export

Export libmbdx data to parquet

```bash
$ brontes db export --help
Usage: brontes db export [OPTIONS]

Options:
  -t, --tables <TABLES>
          Optional tables to exports, if omitted will export all supported tables
          
          [default: MevBlocks AddressMeta SearcherContracts Builder]

  -s, --start-block <START_BLOCK>
          Optional Start Block, if omitted it will export the entire range to parquet

      --brontes-db-path <BRONTES_DB_PATH>
          path to the brontes libmdbx db

  -e, --end-block <END_BLOCK>
          Optional End Block

  -p, --path <PATH>
          Optional path, will default to "data_exports/"

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