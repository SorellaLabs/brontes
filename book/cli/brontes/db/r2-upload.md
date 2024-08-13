# brontes db r2-upload

For internal use only. Uploads snapshots of db every 100k blocks to r2

```bash
$ brontes db r2-upload --help
Usage: brontes db r2-upload [OPTIONS] --r2-config-name <R2_CONFIG_NAME>

Options:
  -r, --r2-config-name <R2_CONFIG_NAME>
          R2 Config Name

  -s, --start-block <START_BLOCK>
          Start Block

      --brontes-db-path <BRONTES_DB_PATH>
          path to the brontes libmdbx db

  -p, --partition-db-folder <PARTITION_DB_FOLDER>
          Path to db partition folder
          
          [default: <CACHE_DIR>-db-partitions/]

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