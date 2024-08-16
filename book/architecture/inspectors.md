# Inspectors

Inspectors are run at the final stage of the block pipeline. Each Inspector applies its own specialized logic to analyze a block, using the [`BlockTree`](./tree.md#block-tree) and [`Metadata`](./database/database.md#1-block-specific-data) provided during execution. Defined as a trait, `Inspectors` allow developers to build custom implementations tailored to their analytical needs.

## Inspector Trait

The `Inspector` trait defines the `inspect_block` method, where you implement your analysis logic. This method accepts [`BlockTree`](./tree.md#block-tree) and [`Metadata`](./database/database.md#1-block-specific-data) as inputs and returns a `Result` type that you specify, allowing you to customize the output to meet your analytical needs.

```rust,ignore
#[async_trait::async_trait]
pub trait Inspector: Send + Sync {
    type Result: Send + Sync;

    async fn inspect_block(
        &self,
        tree: Arc<BlockTree<Action>>,
        metadata: Arc<Metadata>,
    ) -> Self::Result;
}
```

## Mev Inspectors

The `brontes_inspect` crate includes several MEV-inspectors, each implementing the Inspector trait to identify specific MEV types. Follow the links below to learn more about each their methodologies:

- [Cex-Dex Arbitrage](../mev_inspectors/cex-dex-quotes.md)
- [Sandwich Attacks](../mev_inspectors/sandwich.md)
- [Atomic Arbitrage](../mev_inspectors/atomic-arb.md)
- [JIT Liquidity](../mev_inspectors/jit-liquidity.md)
- [Liquidation](../mev_inspectors/liquidation.md)

## Workflow of Default Inspectors

The default inspector workflow is as follows:

<div style="text-align: center;">
 <img src="diagrams/inspector-flow.png" alt="brontes-flow" style="border-radius: 20px; width: 600px; height: auto;">
</div>

### Step 1: Run All Inspectors

All specialized inspectors are run in parallel.

### Step 2: Compose & Filter MEV Results

Once all inspectors have completed their analysis we attempt to compose MEV results & filter duplicates.

**1: Composition Phase**:

The composition phase integrates results from various inspectors to form complex MEV strategies using the [`MEV_COMPOSABILITY_FILTER`](https://github.com/SorellaLabs/brontes/blob/1448e90a30fb856a77e0d4a2cffc6048eef03056/crates/brontes-inspect/src/composer/composer_filters.rs#L21). This filter specifies combinations of child MEVs—such as Sandwich and JIT—that merge into a more complex parent MEV, like JIT Sandwich, through a designated `ComposeFunction`.

The [`try_compose_mev`](https://github.com/SorellaLabs/brontes/blob/1448e90a30fb856a77e0d4a2cffc6048eef03056/crates/brontes-inspect/src/composer/mod.rs#L209) function applies these rules to the sorted MEV data, seeking out matching transaction hashes among the specified MEV types. When all required child MEV types for a combination are present, they are consolidated into a single, composite parent MEV instance.

**2: Deduplication Phase**:

Inspectors, such as those identifying atomic arbitrages and sandwich attacks, may label the same transaction as different MEV types due to overlapping criteria. For instance, the backrun transaction of a sandwich attack will also appear as a profitable arbitrage opportunity to the atomic arbitrage inspector. To resolve such overlaps we deduplicate inspector results ensuring that each classified MEV bundle is correctly classified.

**How Deduplication Works:**

The [`MEV_DEDUPLICATION_FILTER`](https://github.com/SorellaLabs/brontes/blob/1448e90a30fb856a77e0d4a2cffc6048eef03056/crates/brontes-inspect/src/composer/mev_filters.rs#L32) provides a structured way to prioritize MEV types in scenarios where the classification of a transaction overlap. This filter establishes a hierarchy among detected MEV types, specifying which type should take precedence in the final analysis. For example, in cases involving both atomic backrun and sandwich classifications, the filter dictates that the sandwich type, being more comprehensive, should take precedence over the simpler atomic arbitrage.

### Step 3: Calculate Block Builder PnL

After processing the inspector results, we [calculate the block builder’s PnL](https://github.com/SorellaLabs/brontes/blob/1448e90a30fb856a77e0d4a2cffc6048eef03056/crates/brontes-inspect/src/composer/utils.rs#L195), taking into account their revenues and costs:

- **Revenues:**

  - **Builder Revenue:** Total of all priority fees and tips paid to the builder within the block.
  - **MEV Revenue:** Profits or losses from MEV searchers operated by the builder.

- **Costs:**
  - **Proposer Payments:** ETH paid by the builder to the block proposer.
  - **Transaction Sponsorship:** ETH spent by the builder to [sponsor](https://titanbuilder.substack.com/p/titan-tech-teatime-1) transactions within the block.

> **Note:** Some builders secretly refund parts of priority fees to searchers or order flow generators (tg bots for example). We can't track these kickbacks without knowing the addresses involved. If you have this information, please share it to help us improve our calculations.

### Step 4: Store Results

Finally the resulting [`MevBlock`](./database/schema/mev_blocks.md#mevblock-fields) and [`Vec<Bundles>`](./database/schema/mev_blocks.md#bundle-fields) are written to the database in the `MevBlocks` table.
