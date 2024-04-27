# Brontes Book

- [Introduction](./intro.md)
- [Installation](./installation/installation.md)
- [Run Brontes](./run/run_brontes.md)
- [Architecture Overview](./architecture/intro.md)
  - [BlockTree & Classification](./architecture/tree.md)
  - [Database](./architecture/database.md)
  - [Inspectors](./architecture/inspectors.md)
- [Mev Inspectors Deep Dive](./mev_inspectors/intro.md)

  - [Cex-Dex](./mev_inspectors/cex_dex.md)
  - [Sandwich](./mev_inspectors/sandwich.md)
  - [Just in time Liquidity](./mev_inspectors/jit.md)
  - [Atomic Arbitrage](./mev_inspectors/atomic-arb.md)

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
      - [`brontes db test-traces-init`](./cli/brontes/db/test-traces-init.md)
- [Developers](./developers/developers.md) <!-- CLI_REFERENCE END -->
