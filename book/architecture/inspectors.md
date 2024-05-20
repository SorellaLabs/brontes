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

- [Cex-Dex Arbitrage](../mev_inspectors/cex_dex.md)
- [Sandwich Attacks](../mev_inspectors/sandwich.md)
- [Atomic Arbitrage](../mev_inspectors/atomic-arb.md)
- [JIT Liquidity](../mev_inspectors/jit-liquidity.md)
- [Liquidation](../mev_inspectors/liquidation.md)

## Workflow of Default Inspectors

The default inspector workflow for each block is as follows:

<div style="text-align: center;">
 <img src="diagrams/inspector-flow.png" alt="brontes-flow" style="border-radius: 20px; width: 600px; height: auto;">
</div>

### Step 1: Run All Inspectors

All specialized inspectors are run in parallel.

### Step 2: MEV Filtering & Composition

Once all inspectors have completed their analysis we filter duplicates results & identify more complex MEV strategies. The deduplication stage filters out redundant MEV findings across inspectors, ensuring unique instances are processed. Subsequently, a composition phase integrates related MEV occurrences, using predefined rules in `MEV_COMPOSABILITY_FILTER`, to form complex MEV types. This step is critical for understanding intricate MEV strategies that span multiple transaction types.

### Step 3: Calculate Block Builder PnL

The final output includes calculating the Profit and Loss (PnL) for the block builder, factoring in profits from vertically integrated searchers. This comprehensive accounting helps clarify the builder’s net position by including earnings from controlled MEV searchers, which might offset any apparent losses.

### Step 4: Record Results

The resulting [`MevBlock`](./database/schema/mev_blocks.md#mevblock-fields) and [`Vec<Bundles>`](./database/schema/mev_blocks.md#bundle-fields) are written to the database in the `MevBlocks` table.
