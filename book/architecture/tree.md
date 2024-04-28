# Block Tree

The `BlockTree` decodes, classifies and normalizes a block's raw transaction traces into a collection of `TransactionTrees`, each representing a transaction's complete call hierarchy.

A `TransactionTree` organizes all EVM traces as `Actions`, which form the nodes of the tree. Each `Action` normalizes core DeFi operations—such as swaps, flash loans, and mints—into a standardized format. This approach harmonizes idiosyncrasies between different DeFi protocol implementations, generalizing the representation and structure of core primitives to establish a consistent analytical framework applicable across all protocols.

## Block Tree Building

At a high level, generating the Block Tree involves three primary steps:

<div style="text-align: center;">
 <img src="diagrams/tree-flow.png" alt="brontes-flow" style="border-radius: 20px; width: auto ; height: 600px;">
</div>

1. **Fetching Raw EVM Data**: For a given block, Brontes fetches all `TxTrace` and the corresponding `Header` from the database if they are already available. If not, these traces are generated using a custom `revm-inspector` and the resulting data is stored in the database for future use.

2. **Tree Building**: With this transaction traces and block header, the Block Tree is constructed. Each transaction & it's traces are processed passed to the transaction tree builder, which descends through the transaction call hierarchy, classifying each trace into a normalized action.

3. **Processing**: TODO

## Classification Details

### Action Classification

- **Action Classification**: Calls are analyzed to determine if they represent a recognizable action, such as a token swap or liquidity event. This classification uses dynamic dispatch mechanisms facilitated by proc macros, which route the trace data to the appropriate action classifier based on predefined criteria.

The primary step within the Block Tree’s processing is Action Classification, where raw trace data is converted into `NormalizedActions`. This involves identifying and decoding transaction calls that match specific action signatures, a process facilitated by macro-generated code that ensures accuracy and efficiency.

- **Macro-Generated Dispatch**: Action classifiers are dynamically selected based on the transaction's call data signatures, matched against a comprehensive list of known DeFi actions.

for each tx trace, starting at the root, we descend into the call trace, classifying each call frame: - if the trace is a Call we attempt to classify it into a `Action`. This is done by dispatching

In essence we are retrieving the protocol by querying the database using the `get_protocol` function, then we check if the call data is long enough to contain the selector, if not we return None. We then extract the selector from the call data and create a signature with the protocol byte appended to the end. We then create a const for each classifier that contains the signature of the classifier. We then match the signature of the call data with the signature of the classifiers and if we find a match we call the classifier with the call info, db_tx, block and tx_idx.

We then call decode trace data which return an `Action` & a `DexPriceMsg` if relevant.

### Discovery

After actions are classified, the Discovery phase involves deeper analysis to uncover relationships and interactions between different contracts and transactions within the same block. This may involve detecting complex strategies employed across multiple transactions or calls.

### Complex Classification

Complex classifications handle scenarios where actions are spread across multiple call frames or transactions, requiring contextual analysis beyond simple signature matching. This might include:

- **Multi-Stage Transactions**: Classifying actions that depend on the outcome of previous transactions within the same block.
- **Cross-Protocol Interactions**: Identifying and classifying interactions that span multiple DeFi protocols.
