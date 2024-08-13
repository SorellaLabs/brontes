# brontes db cex-query

Fetches Cex data from the Sorella DB

```bash
$ brontes db cex-query --help
Usage: brontes db cex-query [OPTIONS] --block-number <BLOCK_NUMBER> --token-0 <TOKEN_0> --token-1 <TOKEN_1>

Options:
  -b, --block-number <BLOCK_NUMBER>
          The block number

      --token-0 <TOKEN_0>
          The first token in the pair

      --brontes-db-path <BRONTES_DB_PATH>
          path to the brontes libmdbx db

      --token-1 <TOKEN_1>
          The second token in the pair

  -w, --w-multiplier <W_MULTIPLIER>
          Time window multiplier (expands it)
          
          [default: 1]

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