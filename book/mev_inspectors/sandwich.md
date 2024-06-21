# Sandwich Inspector

## Overview

The Sandwich Inspector is designed to detect and analyze the profitability of Sandwich attacks on Ethereum.

### What is a Sandwich Attack?

A Sandwich attack unfolds in three key steps:

1. **Front-run:** An attacker buys an asset just before a victim's transaction, raising its market price.
2. **Victim Transaction:** The victim purchases the asset at the inflated price.
3. **Back-run:** The attacker sells the asset post-victim transaction, profiting from the price difference.

---

## Methodology

### Step 1: Retrieve Relevant Transactions

The inspector retrieves transactions in the block that involve `swap`, `transfer`, `eth_transfer`, `FlashLoan`, `batch_swap` or `aggregator_swap` actions.

### Step 2: Identify Potential Sandwiches

The Sandwich Inspector runs two parallel functions:

1. `get_possible_sandwich_duplicate_senders`: Finds duplicate EOAs (`from` addresses)
2. `get_possible_sandwich_duplicate_contracts`: Finds duplicate contract addresses (`to` addresses)

Both functions use the same logic to detect sandwich attacks, differing only in their focus. This approach catches attacks executed through repeated contract calls or by the same EOA.

**Here's how these functions work:**

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
         // If this is the first time this contract has been called within this block,
         // insert the tx hash into the map
         Entry::Vacant(duplicate_mev_contract) => {
               duplicate_mev_contract.insert((root.tx_hash, root.head.address));
         }
         // Else get's prev tx hash &  for this sender & replaces it
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
                     // If we do, extend the sandwich, this covers the Big Mac
                     // Sandwich case
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

This function iterates through the transaction tree, identifying transactions with identical contract addresses (`to` addresses) within the block. It collects the transactions sandwiched between and creates a [`PossibleSandwich`](https://github.com/SorellaLabs/brontes/blob/5b1d1b4e30d5c92b2a0bc56ff4dd441aed533681/crates/brontes-inspect/src/mev_inspectors/sandwich/types.rs#L7) struct for each potential sandwich attack scenario detected. The `PossibleSandwich` results, identified by duplicate sender & contract, are deduplicated before proceeding to the next step.

### Step 3: Partitioning Possible Sandwiches

Here's how partitioning works:

- We iterate through victim sets in each sandwich.
- Empty victim sets signal a break between attacks.
- We create new `PossibleSandwich` structs at these breaks.

<div style="text-align: center;">
 <img src="sandwich/partition-sandwich.png" alt="Possible Sandwich Partitioning" style="border-radius: 20px; width: 600px; height: auto;">
</div>

> **Note:** Our partitioning assumes attackers maximize efficiency. Multiple attacker transactions without intervening victims may lead to unexpected results. If you find examples breaking this assumption, please report them for a bounty.

### Step 4: Analyze Possible Sandwich Attacks

#### Checking for pool overlap

<div style="text-align: center;">
 <img src="sandwich/overlap-check.png" alt="Sandwich Pool Overlap Check" style="border-radius: 20px; width: 600px; height: auto;">
</div>

1. Checks that the front run & back run transaction swap on at least one common liquidity pool.
2. For each victim transaction (grouped by EOA), check the victim swaps overlap with pools or tokens from the frontrun and backrun transactions. If 50% of the victim swaps or transfers overlap with the frontrun and backrun transactions, the sandwich is confirmed. Otherwise, we return to the calculate sandwich function & will remove transactions.

<div style="text-align: center;">
 <img src="sandwich/victim-trade-overlap.png" alt="Victim Trade Overlap Check" style="border-radius: 20px; width: 600px; height: auto;">
</div>

#### Recursive Sandwich Verification

For a sandwich that does not contain the necessary overlap we will recursively remove transactions from both the start & end of the possible sandwich to evaluate all possible combinations of transactions until we find a match.

<div style="text-align: center;">
 <img src="sandwich/recursive-check.png" alt="Recursive Sandwich Split" style="border-radius: 20px; width: 600px; height: auto;">
</div>

Note:

- Recursive search will stop after 6 iterations.

**Back Shrink**:

- Remove the last victim set
- Remove the backrun tx
- Calculate Sandwich

**Front Shrink**:

- Remove the first victim set
- Remove the first frontrun tx

- **Transaction Collection**: Collects all relevant swap and transfer actions related to both attackers and victims.
- **Overlap Verification**: Ensures that there is a significant overlap between the actions of attackers and the transactions of victims, confirming the presence of a Sandwich attack.
- **Recursive Verification**: If initial checks are inconclusive, a recursive method removes transactions from consideration, re-evaluating potential sandwiches to find valid attack patterns.

### Step 5: Calculating the Sandwich PnL

Now that we have identified a sandwich, we can easily calculate the PnL by calculating the balance deltas of the searcher addresses. This is the searcher revenue. We then calculate the searcher cost by summing the gas costs of all attacker transactions in the sandwich. The profit is the revenue minus the cost.
