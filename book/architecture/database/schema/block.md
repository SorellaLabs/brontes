# Block Tables

## Block Info Table Schema

--

**Table Name:** `BlockInfo`

**Description:** Stores p2p & mev-boost data for each block.

**Key:** Block number (`u64`)

**Value:** [`BlockMetadataInner`](https://github.com/SorellaLabs/brontes/blob/e9935b20922ffcef21471de888dc9d695bc2bd03/crates/brontes-types/src/db/metadata.rs#L43)

**Fields:**

- **block_hash**:
  - **Type:** `U256`
  - **Description:** The block hash.
- **block_timestamp**:
  - **Type:** `u64`
  - **Description:** Block timestamp.
- **relay_timestamp**:
  - **Type:** `Option<u64>`
  - **Description:** Timestamp when the block was received by the first relay.
- **p2p_timestamp**:
  - **Type:** `Option<u64>`
  - **Description:** Timestamp when the block was first seen by a fibernode.
- **proposer_fee_recipient**:
  - **Type:** `Option<Address>`
  - **Description:** Address of the proposer fee recipient.
- **proposer_mev_reward**:
  - **Type:** `Option<u128>`
  - **Description:** Amount of MEV reward payed to the proposer.
- **private_flow**:
  - **Type:** `Vec<TxHash>`
  - **Description:** List of transaction hashes that were not seen in the mempool by Chainbound fibernodes.

---

## TxTraces Table Schema

**Table Name:** `TxTraces`

**Description:** Contains the transaction traces produced by the custom `revm-inspector` for each block.

**Key:** Block number (`u64`)

**Value:** [`TxTracesInner`](https://github.com/SorellaLabs/brontes/blob/e9935b20922ffcef21471de888dc9d695bc2bd03/crates/brontes-types/src/db/traces.rs#L19)

**Fields:**

- **traces**:
  - **Type:** `Option<Vec<TxTrace>>`
  - **Description:** A block's transaction traces.
