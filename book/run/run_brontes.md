# Run Brontes

This section provides instructions on how to run Brontes and introduces some notable command-line options available to customize its operation.

**To start Brontes after installation:**

```bash
brontes run
```

## Specifying a Block Range

- **Start Block**: The block number from which Brontes begins processing (inclusive). If omitted, Brontes will run at tip until manually stopped, provided you have access to the db API.
- **End Block**: The block number at which Brontes stops processing (exclusive). If omitted, Brontes will run historically and continue at the tip until manually stopped, provided you have access to the db API.

```bash
brontes run --start-block 1234567 --end-block 2345678
```

You can also specify multiple block ranges to be run in parallel by using the `--ranges` flag:

```bash
brontes run --ranges 100-120 750-900 3000-5000
```

### Notable Parameters

- **Quote Asset Address**: This sets the asset used to denominate values in the analysis. The default is USDT (Tether) and we recommend sticking to it. To change the default, use:

```bash
brontes run ... --quote-asset [ASSET_ADDRESS]
```

> **Note**
>
> For a complete list of command-line interface (CLI) options refer to the [CLI reference](../cli/cli.md) section in the documentation.
