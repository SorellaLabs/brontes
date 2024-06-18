# Sandwich Inspector

A Sandwich attacks is a type of MEV strategy where an attacker manipulates the market price of assets on AMMs to extract value from a victim's trade. It involves three steps:

1. **Front-run:** The attacker purchases an asset before the victim's transaction, artificially raising its market price right up to the victim's limit price.
2. **Victim Transaction:** The victim unknowingly buys the asset at this inflated price.
3. **Back-run:** The attacker sells the asset immediately after, correcting the price and securing a profit.

## Sandwich Inspector Methodology

The Sandwich Inspector identifies Sandwich attacks by analyzing the following:

### 1) **Retrieves Relevant Transactions**

The inspector retrieves all transactions containing `swap`, `transfer`, `eth_transfer`, `FlashLoan`, `batch_swap` or `aggregator_swap` actions.

### 2) **Identifies Potential Sandwiches**

Runs `get_possible_sandwich` which runs `get_possible_sandwich_duplicate_senders` and `get_possible_sandwich_duplicate_contracts` in parallel. Both functions iterate through the block to identify transactions with identical senders (from address, i.e the EOA) or contracts (to address, i.e mev attacker contract).

Both functions operate in the same way, let's take the `get_possible_sandwich_duplicate_contracts` as an example:

Tracks the set of `duplicate_mev_contracts`

1. For each transaction in the set of relevant transactions, it checks if the contract or eoa address has been involved in previous transactions within the block.

- If it hasn't it adds it to the `duplicate_mev_contracts` set with the address as key & the tx hash & address as value.
- If it has, it retrieves the previous tx hash of that address from the `duplicate_mev_contracts` and uses that tx hash to retrieve the possible victims tx hash from the set of `possible_victims`. These transactions are all transactions that occurred between the first and current transaction of the address.

For that `duplicate_mev_contracts` address, it checks the `possible_sandwiches` set.

- If the address doesn't have a possible sandwich entry, it creates a [`PossibleSandwich`](https://github.com/SorellaLabs/brontes/blob/5b1d1b4e30d5c92b2a0bc56ff4dd441aed533681/crates/brontes-inspect/src/mev_inspectors/sandwich/types.rs#L7) by adding the first tx to the array of possible frontruns and the current tx as a possible backrun and adds the set of `possible_victims` to the victims array.

- If the address already has a possible sandwich entry, it adds the previous transaction to the array of possible frontruns and the current tx as a possible backrun. It also adds the frontrun_victims to the victims array.

It then sets the previous tx hash in the duplicate_mev_contract map to the current tx hash.

Now that it has matched & updated the state for the current transaction, it adds the current tx hash to to all entries in the `possible_victims` set, this is so that all transactions that happened before the current transaction are now tracking this transaction as a possible victim.

It then adds the current tx hash to the `possible_sandwiches` set with an empty array.

Once it has finished iterating through all transactions, it returns the `possible_sandwiches` set which contains all possible sandwich attacks.

Now that we have the possible sandwich set for both the duplicate contracts & duplicate EOAs, we can deduplicate the results and return the final set of possible sandwich attacks.
