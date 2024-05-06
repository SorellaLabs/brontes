# Block Tables

## Block Info Table Schema

**Table Name:** `BlockInfo`

**Description:** Stores metadata for each blockchain block, including timestamps and proposer information.

**Key:** Block number (`u64`)

**Value:** [`BlockMetadataInner`](https://github.com/SorellaLabs/brontes/blob/e9935b20922ffcef21471de888dc9d695bc2bd03/crates/brontes-types/src/db/metadata.rs#L43)

**Fields:**

- **block_hash**:
  - **Type:** `U256`
  - **Description:** The hash of the block.
- **block_timestamp**:
  - **Type:** `u64`
  - **Description:** Timestamp when the block was mined.
- **relay_timestamp**:
  - **Type:** `Option<u64>`
  - **Description:** Timestamp when the block was received by a relay.
- **p2p_timestamp**:
  - **Type:** `Option<u64>`
  - **Description:** Timestamp when the block was first seen by peers.
- **proposer_fee_recipient**:
  - **Type:** `Option<Address>`
  - **Description:** Address of the proposer receiving transaction fees.
- **proposer_mev_reward**:
  - **Type:** `Option<u128>`
  - **Description:** Amount of MEV reward claimed by the proposer.
- **private_flow**:
  - **Type:** `Vec<TxHash>`
  - **Description:** List of transaction hashes that were part of private transactions in the block.

---

## TxTraces Table Schema

**Table Name:** `TxTraces`

**Description:** Contains the transaction traces for each block, providing detailed insights into transaction executions.

**Key:** Block number (`u64`)

**Value:** [`TxTracesInner`](https://github.com/SorellaLabs/brontes/blob/e9935b20922ffcef21471de888dc9d695bc2bd03/crates/brontes-types/src/db/traces.rs#L19)

**Fields:**

- **traces**:
  - **Type:** `Option<Vec<TxTrace>>`
  - **Description:** Detailed traces of transactions within the block.
