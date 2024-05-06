# MevBlocks Table

**Table Name:** `MevBlocks`

**Description:** This table stores the output of Brontes' analytics pipeline.

**Key:** Block number (`u64`)

- **Type:** `u64`
- **Description:** Block number.

**Value:** [`MevBlockWithClassified`](https://github.com/SorellaLabs/brontes/blob/e9935b20922ffcef21471de888dc9d695bc2bd03/crates/brontes-types/src/db/mev_block.rs#L9)

- **Description:** Contains MEV info at the block level and a list of MEV bundles detected within the block.

**Fields:**

- **block**:
  - **Type:** [`MevBlock`](https://github.com/SorellaLabs/brontes/blob/e9935b20922ffcef21471de888dc9d695bc2bd03/crates/brontes-types/src/mev/block.rs#L29)
  - **Description:** General information about the block, including MEV-related metrics and builder mev & non mev profits.
- **mev**:
  - **Type:** `Vec<Bundle>`
  - **Description:** A list of mev bundles identified within the block.
  - **Permalink:** [Bundle Structure](https://github.com/SorellaLabs/brontes/blob/e9935b20922ffcef21471de888dc9d695bc2bd03/crates/brontes-types/src/mev/bundle/mod.rs#L30)

## MevBlock Fields Detailed

- **block_hash**:
  - **Type:** `B256`
  - **Description:** Block hash.
- **block_number**:
  - **Type:** `u64`
  - **Description:** Block number.
- **mev_count**:
  - **Type:** `MevCount`
  - **Description:** A count of various types of MEV bundles detected.
- **eth_price**
  - **Description:** The CEX price of ETH when the block was produced.
- **cumulative_gas_used**
  - **Description:** The total gas used in the block.
- **cumulative_priority_fee**
  - **Description:** The total priority fee paid in the block.
- **total_bribe**
  - **Description:** The total direct builder payment in the block.
- **cumulative_mev_priority_fee_paid**:
  - **Description:** The total priority fee paid by MEV bundles in the block.
- **builder_address**:
  - **Type:** `Address`
  - **Description:** Address of the block builder.
- **builder_eth_profit**
  - **Description:** Builder PnL in ETH.
- **builder_profit_usd**
  - **Description:** Builder PnL in USD.
- **builder_mev_profit_usd**

  - **Description:** Vertically integrated searcher PnL in USD.

- **proposer_fee_recipient**:
  - **Type:** `Option<Address>`
  - **Description:** Proposer fee recipient address.
- **proposer_mev_reward**
  - **Description:** Proposer MEV reward queried from the relay data API.
- **proposer_profit_usd**
  - **Description:** Proposer PnL in USD.
- **cumulative_mev_profit_usd**
  - **Description:** Cumulative MEV profit of all MEV bundles in the block.

### Bundle Fields Detailed

- **header**:
  - **Type:** `BundleHeader`
  - **Description:** Header information of the MEV bundle
- **data**:
  - **Type:** `BundleData`
  - **Description:** The data content of the MEV bundle
