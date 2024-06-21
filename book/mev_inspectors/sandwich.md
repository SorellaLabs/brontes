# Sandwich Inspector

## Sandwich Inspector Methodology

The Sandwich Inspector identifies Sandwich attacks by:

### 1 **Retrieves Relevant Transactions**

### 2 **Identifies Potential Sandwiches**

Executes `get_possible_sandwich` which runs `get_possible_sandwich_duplicate_senders` and `get_possible_sandwich_duplicate_contracts` in parallel. Both functions iterate through the block to identify transactions with identical senders (from address, i.e the EOA) or contracts (to address, i.e mev attacker contract).

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

To refine and streamline the documentation for the Sandwich Inspector in the style of William Zinsser—emphasizing clarity, brevity, and the removal of unnecessary detail—here is a suggested rewrite of the documentation:

---

# Sandwich Inspector

## Overview

The Sandwich Inspector is a tool designed to detect and analyze Sandwich attacks on Ethereum, where attackers manipulate asset prices on Automated Market Makers (AMMs) to extract value from unsuspecting victims.

### What is a Sandwich Attack?

A Sandwich attack unfolds in three key steps:

1. **Front-run:** An attacker buys an asset just before a victim's transaction, raising its market price.
2. **Victim Transaction:** The victim purchases the asset at the inflated price.
3. **Back-run:** The attacker sells the asset post-victim transaction, profiting from the price difference.

## Methodology

### Step 1: Retrieve Relevant Transactions

The inspector retrieves transactions in the block that involve `swap`, `transfer`, `eth_transfer`, `FlashLoan`, `batch_swap` or `aggregator_swap` actions.

### Step 2: Identify Potential Sandwiches

The Sandwich Inspector searches for potential sandwich attacks by running `get_possible_sandwich_duplicate_senders` and `get_possible_sandwich_duplicate_contracts` in parallel

```rust,ignore
fn get_possible_sandwich_duplicate_contracts(
    tree: Arc<BlockTree<Action>>,
) -> Vec<PossibleSandwich> {
   let mut duplicate_contracts: HashMap<Address, (B256, Address)> = HashMap::dfault();
   let mut possible_victims: HashMap<B256, Vec<B256>> = HashMap::default();
   let mut possible_sandwiches: HashMap<Address, PossibleSandwich> = HashMap::dfault();

   for root in tree.tx_roots.iter() {
      // Skip if the tx reverted
      if root.get_root_action().is_revert() {
         continue
      }

      match duplicate_mev_contracts.entry(root.get_to_address()) {
         // If this contract has not been called within this block,
         // insert the tx hash into the map
         Entry::Vacant(duplicate_mev_contract) => {
               duplicate_mev_contract.insert((root.tx_hash, root.head.address));
         }
         // Get's prev tx hash &  for this sender & replaces it
         // with the current tx hash
         Entry::Occupied(mut duplicate_mev_contract) => {
               let (prev_tx_hash, frontrun_eoa) = duplicate_mev_contract.get_mut();

               // If the previous transaction from this contract had possible victims
               // check if we already have a possible sandwich for it. This handles
               // the big mac sandwich case
               if let Some(frontrun_victims) = possible_victims.remove(prev_tx_hash) {
                  // If we don't have a possible sandwich for this contract then
                  // create one
                  match possible_sandwiches.entry(root.get_to_address()) {
                     Entry::Vacant(e) => {
                           e.insert(PossibleSandwich {
                              eoa:                   *frontrun_eoa,
                              possible_frontruns:    vec![*prev_tx_hash],
                              possible_backrun:      root.tx_hash,
                              mev_executor_contract: root.get_to_address(),
                              victims:               vec![frontrun_victims],
                           });
                     }
                     // If we do, extend the sandwich
                     Entry::Occupied(mut o) => {
                           let sandwich = o.get_mut();
                           sandwich.possible_frontruns.push(*prev_tx_hash);
                           sandwich.possible_backrun = root.tx_hash;
                           sandwich.victims.push(frontrun_victims);
                     }
                  }
               }
               // Sets the previous tx hash in the duplicate_mev_contract map to
               // the current tx hash
               *prev_tx_hash = root.tx_hash;
         }
      }

      // Now, for each existing entry in possible_victims, we add the current
      // transaction hash as a potential victim, if it is not the same as
      // the key (which represents another transaction hash)
      for (_, v) in possible_victims.iter_mut() {
         v.push(root.tx_hash);
      }

      possible_victims.insert(root.tx_hash, vec![]);
   }

   possible_sandwiches.into_values().collect()
}
```

**Activity Logging**:

- **New Entries**: Transactions involving addresses not previously recorded are logged with their details into `duplicate_mev_contracts`.
- **Repeated Activity**: For addresses seen before, the system retrieves the most recent transaction associated with that address. This helps in identifying potential victims—transactions executed between the current and previous activities of that address.

**Constructing Sandwich Scenarios**:

- **Fresh Cases**: If an address is implicated in a possible attack for the first time, a new `PossibleSandwich` entry is created. This logs the transaction as an initial front-run attempt, with subsequent transactions potentially serving as back-runs.
- **Ongoing Cases**: For addresses with existing records, the latest transaction is added to the documented list of potential front-runs and considered for back-running, updating the victim list as necessary.

**Continuous Updates**:

- Following the evaluation of each transaction, updates are necessary to maintain the accuracy of ongoing analyses:
  - **Transaction Hash Update**: The system updates its record with the latest transaction hash for each involved address.
  - **Victim Tracking**: It marks the current transaction as a potential victim in the analyses of all preceding transactions.
  - **Sandwich Record Update**: Entries in `possible_sandwiches` are refreshed, setting the stage for the final assessment.

#### Deduplication and Refined Analysis

- After processing, potential sandwiches are deduplicated to ensure that each analyzed scenario is unique, enhancing the clarity and precision of the findings.
- The inspector then segments identified sandwiches based on their transaction flows and victim interactions, allowing for an accurate depiction of distinct attack strategies.

### Deduplication and Partitioning

Once potential sandwiches are identified from both senders and contracts, the results are deduplicated and partitioned to ensure that distinct attacks are separated from continuous transaction flows:

- **Deduplication**: Merges results and removes duplicates to clean up data for accurate analysis.
- **Partitioning**: Segments the identified sandwiches based on gaps in victim transactions to delineate separate attack incidents.

<div style="text-align: center;">
 <img src="sandwich/partition-sandwich.png" alt="Possible Sandwich Partitioning" style="border-radius: 20px; width: 600px; height: auto;">
</div>

> **Note:**
>
> If for some reason the attacker has multiple transactions with no victims in between, the partitioning
> will not work as expected. See example below.

- Possible Frontrun A
- Victim 1
- Victim 2
- Possible Frontrun B
- Possible Frontrun C
- Victim 3
- Victim 4
- Possible Backrun D

The actual sandwich could actually be:

- First Frontrun A
- Victim 1
- Victim 2
- Unrelated or misc attacker transaction: B
- Third Frontrun: C
- Victim 3
- Victim 4
- Backrun: D

However we are operating under the assumption that attackers are maximally efficient & have no reason to endure the gas overhead. If you find an example of a sandwich attack that breaks this assumption please let us know, we'll give you a bounty.

### Step 3: Analyze Sandwich Structures

For each delineated Sandwich structure, a deeper analysis is conducted:

- **Transaction Collection**: Collects all relevant swap and transfer actions related to both attackers and victims.
- **Overlap Verification**: Ensures that there is a significant overlap between the actions of attackers and the transactions of victims, confirming the presence of a Sandwich attack.
- **Recursive Verification**: If initial checks are inconclusive, a recursive method removes transactions from consideration, re-evaluating potential sandwiches to find valid attack patterns.

### Step 4: Calculate Profit

Finally, the profit from identified Sandwich attacks is calculated by assessing the financial impact on the victims versus the gains of the attackers, factoring in transaction costs and market movements.
