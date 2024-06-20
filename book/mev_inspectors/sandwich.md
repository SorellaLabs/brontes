# Sandwich Inspector

A Sandwich attacks is a type of MEV strategy where an attacker manipulates the market price of assets on AMMs to extract value from a victim's trade. It involves three steps:

1. **Front-run:** The attacker purchases an asset before the victim's transaction, artificially raising its market price right up to the victim's limit price.
2. **Victim Transaction:** The victim unknowingly buys the asset at this inflated price.
3. **Back-run:** The attacker sells the asset immediately after, correcting the price and securing a profit.

## Sandwich Inspector Methodology

The Sandwich Inspector identifies Sandwich attacks by analyzing the following:

### 1 **Retrieves Relevant Transactions**

The inspector retrieves all transactions containing `swap`, `transfer`, `eth_transfer`, `FlashLoan`, `batch_swap` or `aggregator_swap` actions.

### 2 **Identifies Potential Sandwiches**

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

#### Possible Sandwich deduplication

Now that we have the possible sandwich set by duplicate contracts & duplicate EOAs, the results need to be deduplicated. The results are chained into a single iterator & deduplicated. Upon deduplication each `PossibleSandwich` is then partitioned.

##### Partitioning Sandwiches

1. Iterates over the victims sets.

- For each victim set if the victim set isn't empty push it to the `victim_set` array.
- If it is empty, there are no victims between the previous attacker tx and the current attacker tx, which implies that these are probably two separate sandwich attacks so we break it up by creating a `PossibleSandwich` that takes as possible frontruns all possible frontruns up to the current tx index & as possible backrun the current tx.

Take the example where we have the following transactions:

Possible Frontruns: [A, B, C]
Possible Backrun: D
Victims: [[1,2], [], [3,4]]

This would get partitioned into two sandwiches:

1. Possible Frontruns: A
   Possible Backrun: B
   Victims: [1, 2]

2. Possible Frontruns: C
   Possible Backrun: D
   Victims: [3, 4]

The caveat to this methodology is that if for some reason the attacker has multiple transactions in a row, the partitioning will not work as expected. For example let's take the same example as above:

Possible Frontruns: [A, B, C]
Possible Backrun: D
Victims: [[1,2], [], [3,4]]

The actual sandwich could actually be:

1. First Frontrun: A
   Victims: [1, 2]
   Unrelated or misc attacker transaction: B
   Third Frontrun: C
   Victims: [3, 4]
   Backrun: D

However we are operating under the assumption that attackers are maximally efficient & have no reason to endure the gas overhead. If you find an example of a sandwich attack that breaks this assumption please let us know, we'll give you a bounty.

Now that the sandwiches have been partitioned, we fetch the `TxInfo` for all transactions in the sandwiches. If their are more than 10 victim sets for a possible sandwich we discard the sandwich for performance reasons. If anyone can find an example of a sandwich attack that breaks these parameters please let us know, we'll give you a bounty.

### 3 **Calculating The Sandwich**

To prepare for the calculation the inspector:

- collects all swaps and transfers for the victim transactions in the possible sandwich
- collects all swap and transfer actions by the searcher in the possible sandwich

Now that this data is collected we start evaluating if the `PossibleSandwich` is an actual sandwich.

First we check check that the suspected sandwicher is using the same EOA for all its transaction or is using an `mev_contract` that is to say a contract that is not verified or classified. This filters out false positives.

<div style="text-align: center;">
 <img src="sandwich/overlap-check.png" alt="Sandwich Pool Overlap Check" style="border-radius: 20px; width: 600px; height: auto;">
</div>

###### Checking for pool overlap

1. Identifies the pools and tokens involved in the frontrun transactions for the possible sandwich, using the classified swaps & transfers to ensure we don't miss sandwiches for DEXs that are not yet supported.
2. Identifies the pools and tokens involved in the backrun transaction in the same way.
3. Checks that the front run & back run swaps intersect on at least one pool.
4. For each victim (grouped by EOA), check the victim swaps or transfers overlap with pools or tokens from the frontrun and backrun transactions. If 50% of the victim swaps or transfers overlap with the frontrun and backrun transactions, the sandwich is confirmed. Otherwise, we return to the calculate sandwich function & will remove transactions.

<div style="text-align: center;">
 <img src="sandwich/victim-trade-overlap.png" alt="Victim Trade Overlap Check" style="border-radius: 20px; width: 600px; height: auto;">
</div>

**Recursive Sandwich Verification**

For a sandwich that does not contain the necessary overlap we will recursively remove transactions from both the start & end of the possible sandwich to evaluate all possible combinations of transactions until we find a match.

Note:

- Recursive search will stop after 6 iterations.

**Back Shrink**:

- Remove the last victim set
- Remove the backrun tx
- Calculate Sandwich

**Front Shrink**:

- Remove the first victim set
- Remove the first frontrun tx

<div style="text-align: center;">
 <img src="sandwich/recursive-check.png" alt="Recursive Sandwich Split" style="border-radius: 20px; width: 600px; height: auto;">
</div>

### 4 **Calculating The Profit**

Now that we have identified a sandwich, we can easily calculate the PnL by calculating the balance deltas of the searcher addresses. This is the searcher revenue. We then calculate the searcher cost by summing the gas costs of all attacker transactions in the sandwich. The profit is the revenue minus the cost.
