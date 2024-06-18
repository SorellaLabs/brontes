# Sandwich Inspector

A Sandwich attacks is a type of MEV strategy where an attacker manipulates the market price of assets on AMMs to extract value from a victim's trade. It involves three steps:

1. **Front-run:** The attacker purchases an asset before the victim's transaction, artificially raising its market price right up to the victim's limit price.
2. **Victim Transaction:** The victim unknowingly buys the asset at this inflated price.
3. **Back-run:** The attacker sells the asset immediately after, correcting the price and securing a profit.

## Sandwich Inspector Methodology

The Sandwich Inspector identifies Sandwich attacks by analyzing the following:

1. **Retrieves Relevant Transactions:** The inspector retrieves all transactions containing `swap`, `transfer`, `eth_transfer`, `FlashLoan`, `batch_swap` or `aggregator_swap` actions.

2. **Identifies Potential Sandwiches:**:

Runs `get_possible_sandwich` which runs `get_possible_sandwich_duplicate_senders` and `get_possible_sandwich_duplicate_contracts` in parallel. Both functions iterate through the block to identify transactions with identical senders (from address, i.e the EOA) or contracts (to address, i.e mev attacker contract).

The function does the following:

1. For each transaction in the set of relevant transactions, it checks if the contract or eoa address has been involved in previous transactions within the block.

- If it hasn't it adds it to the `duplicate_senders` or `duplicate_contracts` set with the address as key & the tx hash & address as value. It adds an empty vec to the set of `possible_victims` with the tx hash as key.
- If it has, it retrieves the previous tx hash of that address from the set of `duplicate_senders` or `duplicate_contracts` and uses that tx hash to retrieve the possible victims tx hash from the set of `possible_victims`. These transactions are all transactions that occurred between the first and current transaction of the address.

For that `duplicate_sender` or `duplicate_contract` address, it checks the `possible_sandwiches` set.

- If the address doesn't have a possible sandwich entry, it creates a [`PossibleSandwich`](the first tx as to the array of possible frontruns and adds the current tx as a possible backrun and adds the set of `possible_victims` to the victims array.

-
