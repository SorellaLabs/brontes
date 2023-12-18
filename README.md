![](assets/Brontes.png)

# Brontes

Sorella's bespoke mev tracing system for Ethereum (and other EVM-compatible chains).

Brontes is a blazingly fast MEV tracing system developed by Sorella Labs, for Ethereum and other EVM-compatible blockchains. It can be run locally or remotely, and is capable of detecting mev in real-time.

## How Brontes Works

Brontes' runs a pipeline for each bock consisting of the following steps:

### 1. Block Tracing

Brontes, directly reads from reth's db to trace a block. The system is also capable of operating remotely via HTTP, though local db connection is highly recommended.

### 2. Tree Building & Meta-Data Fetching

Once a block is traced, Brontes constructs a tree of all transactions within that block, including all subsequent transaction traces. It is a this point that the metadata is also fetched:

- Transaction level dex-pricing
- Centralized exchange pricing
- Private transaction set (using chainbound's mempool indexing) huge s/o to [chainbound](https://www.chainbound.io/) they are the best people to talk to for any p2p / mempool needs!

Initially, this metadata is downloaded via Sorella's API. Subsequently, it's stored locally in Libmdbx for rapid access in future analyses. Optionally, Dex pricing can be computed locally, this comes in very handy when adding support for new dexes.

### 3. Normalization

The tree is then classified, transaction traces are grouped into normalized actions such as:

- NormalizedSwap
- NormalizedMint
- NormalizedTransfer
- NormalizedLiquidation
- NormalizedFlashLoan

This step enables us to flatten out various idiosyncrasies of the different DeFi protocol implementations, so we can generalize them into a single action type.

### 4. Inspection

Utilizing the normalized data, our inspectors process the classified block tree and classifies various forms of MEV.

Today we have inspectors for:

- cex-dex
- sandwich
- liquidation
- atomic arbitrage
- JIT

The inspectors in Brontes are highly modular. By implementing the Inspector trait, developers & researchers can easily integrate additional inspectors into the system.

```rust
#[async_trait::async_trait]
pub trait Inspector: Send + Sync {
    async fn process_tree(
        &self,
        tree: Arc<TimeTree<Actions>>,
        metadata: Arc<Metadata>,
    ) -> Vec<(ClassifiedMev, Box<dyn SpecificMev>)>;
}
```

### 5. Composition

Finally, the individual inspectors results are collected by the composer, a higher level inspector that tries to identify more complex MEV strategies that are composed of multiple individual MEV actions.

Such as:

- JIT + sandwich
