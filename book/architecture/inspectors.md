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

The default inspector workflow is as follows:

<div style="text-align: center;">
 <img src="diagrams/inspector-flow.png" alt="brontes-flow" style="border-radius: 20px; width: 600px; height: auto;">
</div>

### Step 1: Run All Inspectors

All specialized inspectors are run in parallel.

### Step 2: MEV Filtering & Composition

Once all inspectors have completed their analysis we attempt to compose MEV results & filter duplicates.

**1: Composition Phase**:

The composition phase integrates results from various inspectors to identify more complex MEV strategies. Using the `MEV_COMPOSABILITY_FILTER`, we map multiple types of child MEV into a single, complex parent MEV. This filter defines specific combinations where child MEVs, such as Sandwich and JIT, are combined into a new parent MEV instance, JIT Sandwich, based on predefined rules.

The `try_compose_mev` function examines the sorted MEV data to identify and merge compatible MEV instances. It checks for matching transaction hashes among MEV types listed in the filter. When a complete set of required child MEVs is found, they are combined into a new parent MEV.

**2: Deduplication Phase**:

Inspectors, such as those identifying atomic arbitrages and sandwich attacks, may label the same transaction as different MEV types due to overlapping criteria. For instance, the backrun transaction of a sandwich attack will also appear as a profitable arbitrage opportunity to the atomic arbitrage inspector. To resolve such overlaps we deduplicate inspector results ensuring that each classified MEV bundle is correctly classified.

**How Deduplication Works:**

- The `MEV_DEDUPLICATION_FILTER` provides a structured way to prioritize MEV types in scenarios where the classification of a transaction overlap. This filter establishes a hierarchy among detected MEV types, specifying which type should take precedence in the final analysis. For example, in cases involving both atomic backrun and sandwich classifications, the filter dictates that the sandwich type, being more comprehensive, should take precedence over the simpler atomic arbitrage. Below you can see the complete set of precedence rules applied in our deduplication process:

```rust,ignore
define_mev_precedence!(
    Unknown, SearcherTx => CexDex;
    Unknown, SearcherTx, CexDex => AtomicArb;
    Unknown, SearcherTx, AtomicArb, CexDex => Liquidation;
    Unknown, SearcherTx, AtomicArb, CexDex => Sandwich;
    Unknown, SearcherTx, AtomicArb, CexDex, Sandwich => Jit;
    Unknown, SearcherTx, AtomicArb, CexDex, Jit, Sandwich => JitSandwich;
);
```

### Why Deduplication is Necessary:

### How Deduplication Works:

The `MEV_DEDUPLICATION_FILTER` plays a crucial role by establishing a hierarchy among MEV types, ensuring that in cases of overlap, more comprehensive categories take precedence. For example, when both atomic backrun and sandwich detections occur for the same transaction, the filter specifies that the sandwich type, being more encompassing, should override the atomic arbitrage. This structured prioritization avoids the double counting of transactions under multiple labels and refines the analysis, presenting a clearer and more meaningful representation of economic activities within a block.

By streamlining the deduplication process, we ensure that the data not only accurately reflects each unique MEV occurrence but also provides stakeholders with a reliable understanding of the frequency and impact of various MEV strategies, unclouded by overlapping detections. This clarity is vital for accurately gauging the blockchain's complex dynamics and the economic implications of MEV strategies.

### Step 3: Calculate Block Builder PnL

The final output includes calculating the Profit and Loss (PnL) for the block builder, factoring in profits from vertically integrated searchers. This comprehensive accounting helps clarify the builder’s net position by including earnings from controlled MEV searchers, which might offset any apparent losses.

### Step 4: Record Results

The resulting [`MevBlock`](./database/schema/mev_blocks.md#mevblock-fields) and [`Vec<Bundles>`](./database/schema/mev_blocks.md#bundle-fields) are written to the database in the `MevBlocks` table.

```

```
