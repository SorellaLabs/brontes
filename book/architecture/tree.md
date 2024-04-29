# Block Tree

The `BlockTree` decodes, classifies and normalizes a block's transaction traces into a collection of `TransactionTrees`, each representing a transaction's call hierarchy.

A `TransactionTree` organizes all EVM traces as `Actions`, which form the nodes of the tree. Each `Action` normalizes core DeFi operations—such as swaps, flash loans, and mints—into a standardized format. This approach harmonizes idiosyncrasies between different DeFi protocol implementations, generalizing the representation of core primitives to establish a consistent analytical framework applicable across all protocols.

## Block Tree Building

At a high level, generating the Block Tree involves three primary steps:

<div style="text-align: center;">
 <img src="diagrams/tree-flow.png" alt="brontes-flow" style="border-radius: 20px; width: auto ; height: 600px;">
</div>

1. **Fetching Raw EVM Data**: Brontes retrieves the transaction traces and the `BlockHeader` for a block, first querying the database. If the data is not available, it is generated using a custom `revm-inspector` and stored to accelerate reruns.

2. **Tree Building**: Traced transactions are individually passed to the TxTree builder which descends through the call hierarchy, classifying each trace into a normalized action. Decoding and normalization occur via the `dispatch` macro which routes call data to it's protocol classifier. See the [Action Classification](#action-classification) section for more.

3. **Processing**: The newly built BlockTree undergoes sanitization to account for tax tokens and duplicate transfers. It also classifies multi-call frame actions, which span multiple traces. More on this in the [Complex Classification](#complex-classification) section.

## Action Classification

Each transaction trace is decoded and labelled into a `NormalizedAction` by

- **Action Classification**: Calls are analyzed to determine if they represent a recognizable action, such as a token swap or liquidity event. This classification uses dynamic dispatch mechanisms facilitated by proc macros, which route the trace data to the appropriate action classifier based on predefined criteria.

The primary step within the Block Tree’s processing is Action Classification, where raw trace data is converted into `NormalizedActions`. This involves identifying and decoding transaction calls that match specific action signatures, a process facilitated by macro-generated code that ensures accuracy and efficiency.

- **Macro-Generated Dispatch**: Action classifiers are dynamically selected based on the transaction's call data signatures, matched against a comprehensive list of known DeFi actions.
  `for each tx trace, starting at the root, we descend into the call trace, classifying each call frame: - if the trace is a Call we attempt to classify it into a`Action`. This is done by dispatching

In essence we are retrieving the protocol by querying the database using the `get_protocol` function, then we check if the call data is long enough to contain the selector, if not we return None. We then extract the selector from the call data and create a signature with the protocol byte appended to the end. We then create a const for each classifier that contains the signature of the classifier. We then match the signature of the call data with the signature of the classifiers and if we find a match we call the classifier with the call info, db_tx, block and tx_idx.

We then call decode trace data which return an `Action` & a `DexPriceMsg` if relevant.

### Discovery

After actions are classified, the Discovery phase involves deeper analysis to uncover relationships and interactions between different contracts and transactions within the same block. This may involve detecting complex strategies employed across multiple transactions or calls.

### Complex Classification

Complex classifications handle scenarios where actions are spread across multiple call frames or transactions, requiring contextual analysis beyond simple signature matching. This might include:

- **Multi-Stage Transactions**: Classifying actions that depend on the outcome of previous transactions within the same block.
- **Cross-Protocol Interactions**: Identifying and classifying interactions that span multiple DeFi protocols.
