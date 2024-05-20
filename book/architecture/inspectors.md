# Inspectors

Inspectors are run at the final stage of the block pipeline. Each Inspector applies its own specialized logic to analyze a block, using the [`BlockTree`](./tree.md#block-tree) and [`Metadata`](./database/database.md#1-block-specific-data) provided during execution. Defined as a trait, `Inspectors` allow developers to build custom implementations tailored to their analytical needs.

## Inspector Trait

An `Inspector` is a trait defining the `process_tree` method, which takes a `BlockTree` and `Metadata` as input. It returns a result specific to the inspector's implementation, which allows for any type of specialized analysis to be implemented.

```rust,ignore
#[async_trait::async_trait]
pub trait Inspector: Send + Sync {
    type Result: Send + Sync;

    async fn process_tree(
        &self,
        tree: Arc<BlockTree<Action>>,
        metadata: Arc<Metadata>,
    ) -> Self::Result;
}
```

<div style="text-align: center;">
 <img src="diagrams/composer.png" alt="brontes-flow" style="border-radius: 20px; width: 600px; height: auto;">
</div>

### MEV Inspectors

The `brontes_inspect` crate provides several individual inspectors, each designed to detect a specific type of MEV strategy. These inspectors are defined in their respective modules:

- `atomic_backrun`
- `cex_dex`
- `jit`
- `sandwich`
- `liquidations`
- `long_tail`

Each inspector implements the `Inspector` trait, providing its unique implementation of the `process_tree` method.

#### Composer

The `Composer` is a special type of inspector that combines the results of individual inspectors to identify more complex MEV strategies. It takes an array of individual inspectors, a `BlockTree`, and `Metadata` as input, running each inspector on the block and collecting their results.

```rust,ignore
pub struct Composer<'a, const N: usize> {
    inspectors_execution: InspectorFut<'a>,
    pre_processing:       BlockPreprocessing,
}
```

The `Composer` defines a filter to order results from individual inspectors, ensuring lower-level actions are composed before higher-level actions, which could affect the composition. Additionally, the `Composer` provides a `Future` implementation for asynchronous contexts. When polled, it runs the individual inspectors in parallel, collecting and processing their results to identify complex MEV strategies.
