# MevBlocks Table

**Table Name:** `MevBlocks`

**Description:** This table captures data pertaining to Miner Extractable Value (MEV) opportunities within blocks, integrating both general block information and specifics about MEV bundles detected.

**Key:** Block number (`u64`)

- **Type:** `u64`
- **Description:** Identifies the specific block number which the MEV data corresponds to.

**Value:** [`MevBlockWithClassified`](https://github.com/SorellaLabs/brontes/blob/e9935b20922ffcef21471de888dc9d695bc2bd03/crates/brontes-types/src/db/mev_block.rs#L9)

- **Description:** A structure containing detailed information about the block and the MEV bundles within it.

**Fields:**

- **block**:
  - **Type:** [`MevBlock`](https://github.com/SorellaLabs/brontes/blob/e9935b20922ffcef21471de888dc9d695bc2bd03/crates/brontes-types/src/mev/block.rs#L29)
  - **Description:** General information about the block, including MEV-related metrics and builder profits.
- **mev**:
  - **Type:** `Vec<Bundle>`
  - **Description:** A list of bundles representing various MEV opportunities identified within the block.
  - **Permalink:** [Bundle Structure](https://github.com/SorellaLabs/brontes/blob/e9935b20922ffcef21471de888dc9d695bc2bd03/crates/brontes-types/src/mev/bundle/mod.rs#L30)

## MevBlock Fields Detailed

- **block_hash**:
  - **Type:** `B256`
  - **Description:** The unique hash identifying the block.
- **block_number**:
  - **Type:** `u64`
  - **Description:** The number of the block within the blockchain.
- **mev_count**:
  - **Type:** `MevCount`
  - **Description:** A count of various types of MEV scenarios detected.
- **eth_price**, **cumulative_gas_used**, **cumulative_priority_fee**, **total_bribe**, **cumulative_mev_priority_fee_paid**:
  - **Description:** Financial and transactional metrics relevant to MEV analysis.
- **builder_address**:
  - **Type:** `Address`
  - **Description:** Address of the block builder.
- **builder_eth_profit**, **builder_profit_usd**, **builder_mev_profit_usd**:
  - **Description:** Profit metrics for the builder, measured in ETH and USD.
- **proposer_fee_recipient**:
  - **Type:** `Option<Address>`
  - **Description:** Address receiving the transaction fees.
- **proposer_mev_reward**, **proposer_profit_usd**, **cumulative_mev_profit_usd**:
  - **Description:** Monetary rewards and profits accrued to the proposer due to MEV.

### Bundle Fields Detailed

- **header**:
  - **Type:** `BundleHeader`
  - **Description:** Header information of the MEV bundle, outlining key details.
- **data**:
  - **Type:** `BundleData`
  - **Description:** The data content of the MEV bundle, detailing the transactions and actions involved.
