# Brontes Architecture

## How Brontes Works?

Brontes transforms raw Ethereum transaction traces into a structured, analyzable format through a multi-step process:

1. **Block Tracing**: Brontes performs block tracing by reading from Reth's database or operating remotely via HTTP.

2. **Tree Construction**: Brontes constructs a tree of all transactions within a block, encapsulating each transaction in its own `TransactionTree`, which preserves the execution order and context.

3. **Metadata Integration**: In parallel to the tree construction, Brontes fetches and integrates relevant metadata, such as DEX pricing, exchange pricing, and private transaction sets. For more information, see the [database](./architecture/database.md) section.

4. **Normalization**: Brontes employs [Classifiers](./classifiers.md) to normalize the raw traces into standardized `NormalizedActions`, establishing a consistent analytical framework across different DeFi protocols.

5. **Inspection**: The classified blocks, enriched with metadata, are passed to the [Inspector Framework](./inspectors.md). Inspectors process the classified blocks and metadata to identify various forms of MEV. The modular nature of the Inspector Framework allows developers to easily integrate additional inspectors.

6. **Composition**: The individual inspector results are collected by the composer, a higher-level inspector that identifies complex MEV strategies composed of multiple MEV actions.

For a more detailed explanation of each component and instructions on implementing custom classifiers and inspectors, please refer to the following sections:

- [Classifiers](./classifiers.md)
- [Metadata](./metadata.md)
- [Inspector Framework](./inspectors.md)

This version provides a concise, high-level overview of the Brontes pipeline, briefly mentioning the key components and their functions. The references to dedicated sections allow readers to dive deeper into specific topics of interest without overwhelming them with details in the main "How Brontes Works?" section.

At the heart of Brontes is the process of converting raw Ethereum transaction traces into a more digestible structure while preserving crucial contextual information. This journey begins with the raw traces, which are then transformed into classified blocks through a series of steps.

First, Brontes performs generates block traces. Once a block is traced, Brontes constructs a tree of all transactions within that block, encapsulating each transaction in its own `TransactionTree`. A `TransactionTree` represents a transaction in a tree-like structure, with traces represented as nodes, preserving the execution order and context in a structured manner.

In parallel to the tree construction, Brontes fetches and integrates relevant metadata, such as transaction-level DEX pricing, centralized exchange pricing, and private transaction sets.

With the `TransactionTrees` constructed, Brontes moves on to the normalization phase. In this crucial step, raw traces are classified into `NormalizedActions`, which standardize the diverse actions found across DeFi protocols into a unified format. By generalizing core primitives–such as swaps, flash loans, mints, among others—into unified types, Brontes establishes a consistent analytical framework that applies across all protocols for each core action. This normalization process not only organizes data but also harmonizes the idiosyncrasies between different DeFi protocol implementations.

The classified blocks, enriched with metadata and normalized actions, are then passed to the Inspector Framework. This is where the magic of complex analysis happens. Inspectors process the classified blocks and metadata, identifying various forms of MEV, such as CEX-DEX arbitrage, sandwich attacks, liquidations, atomic arbitrage, and just-in-time (JIT) liquidity. The modular nature of the Inspector Framework allows developers to easily integrate additional inspectors by implementing the `Inspector` trait, blissfully unaware of the preprocessing efforts involved.

Finally, the individual inspector results are collected by the composer, a higher-level inspector that attempts to identify more complex MEV strategies composed of multiple individual MEV actions, such as JIT combined with sandwich attacks.
