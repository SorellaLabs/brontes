# MevBlocks Table

---

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

## MevBlock Fields

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

## Bundle Fields

- **header**:
  - **Type:** [`BundleHeader`](https://github.com/SorellaLabs/brontes/blob/5ea4889b848e4c6a4c20b60535c56eb350bd1f5e/crates/brontes-types/src/mev/bundle/header.rs#L31)
  - **Description:** Header information of the MEV bundle
- **data**:
  - **Type:** [`BundleData`](https://github.com/SorellaLabs/brontes/blob/5ea4889b848e4c6a4c20b60535c56eb350bd1f5e/crates/brontes-types/src/mev/bundle/data.rs#L26)
  - **Description:** Enum that encapsulates specific data structures for each type of MEV.

## Bundle Header

**Bundle Header**:
The Bundle Header is common to all MEV types within Brontes. It provides a uniform structure for capturing essential transaction details, enabling the classification and analysis of MEV activities.

**Fields**:

- **block_number**: Identifies the block number where the MEV event occurred.
  - **Type**: `u64`
- **tx_index**: Index of the transaction within the block.
  - **Type**: `u64`
- **tx_hash**: Hash of the transaction involved in the MEV event.
  - **Type**: `B256`
- **eoa**: Address of the externally owned account initiating the transaction.
  - **Type**: `Address`
- **mev_contract**: Optionally, the address of a smart contract involved in the MEV strategy.
  - **Type**: `Option<Address>`
- **profit_usd**: Profit in USD derived from the MEV activity.
  - **Type**: `f64`
- **bribe_usd**: Cost in USD paid as a priority fee or bribe.
  - **Type**: `f64`
- **mev_type**: Categorizes the type of MEV activity.
  - **Type**: `MevType`
  - **Enum Values**: [CexDex, Sandwich, Jit, JitSandwich, Liquidation, AtomicArb, SearcherTx, Unknown](https://github.com/SorellaLabs/brontes/blob/e9935b20922ffcef21471de888dc9d695bc2bd03/crates/brontes-types/src/db/mev_types.rs#L10)
- **no_pricing_calculated**: Indicates if the MEV was calculated without specific pricing models.
  - **Type**: `bool`
- **balance_deltas**: A list of balance changes across different addresses.
  - **Type**: [`Vec<[TransactionAccounting>`](https://github.com/SorellaLabs/brontes/blob/5ea4889b848e4c6a4c20b60535c56eb350bd1f5e/crates/brontes-types/src/mev/bundle/header.rs#L54)

### TransactionAccounting

**Fields**:

- **tx_hash**: Transaction hash where the balance change occurred.
  - **Type**: `B256`
- **address_deltas**: List of balance changes by address.
  - **Type**: [`Vec<AddressBalanceDeltas>`](https://github.com/SorellaLabs/brontes/blob/5ea4889b848e4c6a4c20b60535c56eb350bd1f5e/crates/brontes-types/src/mev/bundle/header.rs#L73)

### AddressBalanceDeltas

**Fields**:

- **address**: Blockchain address experiencing the balance change.
  - **Type**: `Address`
- **name**: Optional name or alias for the address.
  - **Type**: `Option<String>`
- **token_deltas**: Detailed changes in token balances.
  - **Type**: `Vec<TokenBalanceDelta>`

### TokenBalanceDelta

**Fields**:

- **token**: Detailed information about the token.
  - **Type**: `TokenInfoWithAddress`
- **amount**: Amount of the token that has changed.
  - **Type**: `f64`
- **usd_value**: USD value of the token change.
  - **Type**: `f64`

## Bundle Data

Bundle Data is an enumeration that encapsulates specific data structures representing different MEV strategies.

- **Type**: [`BundleData`](https://github.com/SorellaLabs/brontes/blob/5ea4889b848e4c6a4c20b60535c56eb350bd1f5e/crates/brontes-types/src/mev/bundle/data.rs#L26)

```rust,ignore
pub enum BundleData {
    Sandwich(Sandwich),
    AtomicArb(AtomicArb),
    JitSandwich(JitLiquiditySandwich),
    Jit(JitLiquidity),
    CexDex(CexDex),
    Liquidation(Liquidation),
    Unknown(SearcherTx),
}
```

- **Description**: Each variant in the Bundle Data enum represents a distinct type of MEV, with a specific struct that contains the details of the bundle.

### Sandwich

**Description**: Represents a range of sandwich attack strategies, from standard to complex variations. These attacks typically involve a frontrun and a backrun transaction bracketing a victim's trade, exploiting the victim's slippage.

**Type**: [`Sandwich`](https://github.com/SorellaLabs/brontes/blob/5ea4889b848e4c6a4c20b60535c56eb350bd1f5e/crates/brontes-types/src/mev/sandwich.rs#L62)

**Fields**:

- **frontrun_tx_hash**: Hashes of transactions that frontrun the victim.
- **frontrun_swaps**: Details of swaps executed in the frontrunning transactions.
- **victim_swaps_tx_hashes**: Hashes of victim transactions targeted by the frontrun.
- **victim_swaps**: Details of swaps executed by the victim.
- **backrun_tx_hash**: Hash of the transaction that backruns the victim.
- **backrun_swaps**: Details of swaps executed in the backrunning transaction.

### Atomic Arb

**Type**: [`AtomicArb`](https://github.com/SorellaLabs/brontes/blob/5ea4889b848e4c6a4c20b60535c56eb350bd1f5e/crates/brontes-types/src/mev/backrun.rs#L27)

**Description**: Represents arbitrage strategies that exploit price discrepancies across different liquidity pools or exchanges within a single transaction.

**Fields**:

- **tx_hash**: Transaction hash of the arbitrage.
- **swaps**: List of swaps executed to capitalize on the arbitrage opportunity.
- **arb_type**: Type of arbitrage strategy, categorized by complexity and methodology, such as Triangle, CrossPair, StablecoinArb, or LongTail.

### Jit Liquidity

**Type**: [`JitLiquidity`](https://github.com/SorellaLabs/brontes/blob/5ea4889b848e4c6a4c20b60535c56eb350bd1f5e/crates/brontes-types/src/mev/jit.rs#L29)

**Description**: Involves strategies where liquidity is added just-in-time to facilitate trades or other on-chain operations, often to minimize slippage or to setup for subsequent profitable trades.

**Fields**:

- **frontrun_mint_tx_hash**: Hash of transactions adding liquidity.
- **frontrun_mints**: Liquidity additions that precede critical trades.
- **victim_swaps_tx_hashes**: Hashes of trades that utilize the just-added liquidity.
- **victim_swaps**: Details of trades using the added liquidity.
- **backrun_burn_tx_hash**: Hash of transactions removing liquidity post-trade.
- **backrun_burns**: Liquidity removals following the trading activity.

### Jit Sandwich

**Type**: [`JitLiquiditySandwich`](https://github.com/SorellaLabs/brontes/blob/5ea4889b848e4c6a4c20b60535c56eb350bd1f5e/crates/brontes-types/src/mev/jit_sandwich.rs#L28)

**Description**: A combination of JIT liquidity strategies and sandwich attacks, where liquidity is added and removed to exploit and manipulate trade outcomes extensively.

**Fields**:

- **frontrun_tx_hash**: Hashes of transactions that both frontrun a victim and add liquidity.
- **frontrun_swaps**: Swaps executed in the frontrunning phase.
- **frontrun_mints**: Liquidity added in anticipation of victim trades.
- **victim_swaps_tx_hashes**: Hashes of victim transactions.
- **victim_swaps**: Trades executed by the victim.
- **backrun_tx_hash**: Hash of the transaction that removes liquidity and possibly executes backrun swaps.

### Cex Dex

**Type**: [`CexDex`](https://github.com/SorellaLabs/brontes/blob/5ea4889b848e4c6a4c20b60535c56eb350bd1f5e/crates/brontes-types/src/mev/cex_dex.rs#L38)

**Description**: Exploits the price differences between centralized exchanges (CEX) and decentralized exchanges (DEX) for arbitrage opportunities.

**Fields**:

- **tx_hash**: Transaction hash of the arbitrage.
- **swaps**: List of swaps executed across exchanges.
- **global_vmap_details**: Arbitrage details using global VMAP quotes.
- **optimal_route_details**: Arbitrage executed using the most optimal routing across exchanges.

### Liquidation

**Description**: Involves transactions aimed at executing liquidations on over-leveraged positions in DeFi protocols, often involving complex strategies to trigger these liquidations profitably.

**Fields**:

- **liquidation_tx_hash**: Transaction hash of the liquidation.
- **trigger**: Transaction or event that triggered the liquidation.
- **liquidation_swaps**: Swaps executed as part of the liquidation process.

### Unknown (SearcherTx)

**Description**: This category captures MEV-related transactions that do not fit into the standard categories, often involving bespoke or highly specialized strategies.

**Fields**:

- **tx_hash**: Hash of the transaction.
- **transfers**: Details of transfers executed within the transaction, often linked to complex MEV strategies.
