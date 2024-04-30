# Block Tree

The `BlockTree` decodes, classifies and normalizes a block's transaction traces into a collection of `TransactionTrees`, each representing a transaction's call hierarchy.

A `TransactionTree` organizes all EVM traces as a collection of `Action`, which form the nodes of the tree. Each `Action` normalizes core DeFi operations—such as swaps, flash loans, and mints—into a standardized format. This approach harmonizes idiosyncrasies between different DeFi protocol implementations, generalizing the representation of core primitives to establish a consistent analytical framework applicable across all protocols.

## Block Tree Building

At a high level, generating the Block Tree involves three primary steps:

<div style="text-align: center;">
 <img src="diagrams/tree-flow.png" alt="brontes-flow" style="border-radius: 20px; width: auto ; height: 600px;">
</div>

1. **Fetching Raw EVM Data**: Brontes retrieves the transaction traces and the `BlockHeader` for a block, first querying the database. If the data is not available, it is generated using a custom `revm-inspector` and stored to accelerate reruns.

2. **Tree Building**: Traced transactions are individually passed to the TxTree builder which descends through the call hierarchy, classifying each trace into a normalized action. Decoding and normalization occur via the `dispatch` macro which routes call data to it's protocol classifier. See the [Action Classification](#action-classification) section for more.

3. **Processing**: The newly built BlockTree undergoes sanitization to account for tax tokens and duplicate transfers. It also classifies multi-call frame actions, which span multiple traces. More on this in the [Complex Classification](#complex-classification) section.

Trace -> match on trace type -> classify create | classify call -> action classification

## Action Classification

Each transaction trace is classified into an `Action` and an optional `DexPriceMsg` if it should be priced. The diagram below illustrates the classification process:

<div style="text-align: center;">
 <img src="diagrams/trace-classifier.png" alt="brontes-flow" style="border-radius: 10px; width: auto ; height: 600px;">
</div>

### Protocol Classifiers

**Matching to the right classifier**

- Retrieve the protocol by querying the database using the `get_protocol` function,
- Extract the selector from the trace call data and create a match key by appending the protocol byte to the function signature. - match the signature of the call data with the signature of the classifiers and if we find a match we call the classifier with the call info, db_tx, block and tx_idx.

**Classifying the action**

### Discovery Classifiers

After actions are classified, the Discovery phase involves deeper analysis to uncover relationships and interactions between different contracts and transactions within the same block. This may involve detecting complex strategies employed across multiple transactions or calls.

### Implementing a New Classifier

### Complex Classification

Complex classifications handle scenarios where actions are spread across multiple call frames or transactions, requiring contextual analysis beyond simple signature matching. This might include:

- **Multi-Stage Transactions**: Classifying actions that depend on the outcome of previous transactions within the same block.
- **Cross-Protocol Interactions**: Identifying and classifying interactions that span multiple DeFi protocols.
