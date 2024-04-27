# Tree Construction and Normalization

At the heart of Brontes is the process of converting raw Ethereum transaction traces into a more digestible structure while preserving crucial contextual information. This is achieved by creating classified blocks, where each transaction is encapsulated in its own `TransactionTree`. A `TransactionTree` represents a transaction in a tree-like structure, with traces represented as nodes, preserving the execution order and context in a structured manner.

In constructing these `TransactionTrees`, Brontes classifies raw traces into `NormalizedActions`, a crucial step that standardizes the diverse actions found across DeFi protocols into a unified format. This standardization not only organizes data but also harmonizes the idiosyncrasies between different DeFi protocol implementations. By generalizing core primitives–such as swaps, flash loans, mints, among others—into unified types, Brontes establishes a consistent analytical framework that applies across all protocols for each core action.

## Discovery

## Classifiers

TODO: explain classifiers + macro

### Complex Classification

TODO: explain complex classification aka multi call frame classification
