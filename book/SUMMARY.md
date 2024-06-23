# Brontes Book

- [Introduction](./intro.md)
- [Installation](./installation/installation.md)
- [Run Brontes](./run/run_brontes.md)
- [Architecture Overview](./architecture/intro.md)

  - [Block Tree](./architecture/tree.md)
  - [Database](./architecture/database/database.md)

    - [Tables Schema](./architecture/database/tables_schema.md)
    - [Pricing Tables](./architecture/database/schema/pricing.md)
    - [Block Tables](./architecture/database/schema/block.md)
    - [Metadata Tables](./architecture/database/schema/metadata.md)
    - [Classification Tables](./architecture/database/schema/classification.md)
    - [Mev Block Tables](./architecture/database/schema/mev_blocks.md)
    - [Misc Tables](./architecture/database/schema/misc.md)

  - [Inspectors](./architecture/inspectors.md)

- [Inspector Methodology](./mev_inspectors/intro.md)

  - [Cex-Dex Arbitrage](./mev_inspectors/cex_dex.md)
  - [Sandwich Attack](./mev_inspectors/sandwich.md)
  - [Atomic Arbitrage](./mev_inspectors/atomic-arb.md)
  - [JIT Liquidity](./mev_inspectors/jit-liquidity.md)
  - [Liquidation](./mev_inspectors/liquidation.md)

- [CLI Reference](./cli/cli.md) <!-- CLI_REFERENCE START -->
  - [`brontes`](./cli/brontes.md)
    - [`brontes run`](./cli/brontes/run.md)
    - [`brontes db`](./cli/brontes/db.md)
      - [`brontes db insert`](./cli/brontes/db/insert.md)
      - [`brontes db query`](./cli/brontes/db/query.md)
      - [`brontes db clear`](./cli/brontes/db/clear.md)
      - [`brontes db generate-traces`](./cli/brontes/db/generate-traces.md)
      - [`brontes db libmdbx-mem-test`](./cli/brontes/db/libmdbx-mem-test.md)
      - [`brontes db init`](./cli/brontes/db/init.md)
      - [`brontes db export`](./cli/brontes/db/export.md)
      - [`brontes db download-snapshot`](./cli/brontes/db/download-snapshot.md)
- [Developers](./developers/developers.md) <!-- CLI_REFERENCE END -->
