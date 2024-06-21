# Sandwich Inspector

## Overview

The Sandwich Inspector is designed to detect and analyze the profitability of Sandwich attacks.

### What is a Sandwich Attack?

A Sandwich attack unfolds in three steps:

1. **Front-run:** An attacker buys an asset just before a victim's transaction, raising its market price.
2. **Victim Transaction:** The victim purchases the asset at the inflated price.
3. **Back-run:** The attacker sells the asset post-victim transaction, profiting from the price difference.

## Methodology

### Step 1: Retrieve Relevant Transactions

The inspector retrieves transactions in the block that involve `swap`, `transfer`, `eth_transfer`, `FlashLoan`, `batch_swap` or `aggregator_swap` actions.

### Step 2: Identify Potential Sandwiches

The Sandwich Inspector runs two functions in parallel:

1. `get_possible_sandwich_duplicate_senders`: Finds duplicate EOAs (`from` addresses)
2. `get_possible_sandwich_duplicate_contracts`: Finds duplicate contract addresses (`to` addresses)

Both functions use the same logic to detect possible sandwich attacks, differing only in their focus. This approach catches attacks executed through repeated contract calls or by the same EOA.

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
         // Else get the previous tx hash for this contract & replaces it
         // with the current tx hash
         Entry::Occupied(mut duplicate_mev_contract) => {
               let (prev_tx_hash, frontrun_eoa) = duplicate_mev_contract.get_mut();

               // If the previous transaction calling this contract had possible victims
               // check if we already have a possible sandwich for it.
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

      // For each transaction we've inspected, add the current
      // transaction hash as a potential victim
      for (_, v) in possible_victims.iter_mut() {
         v.push(root.tx_hash);
      }

      // Insert the current transaction hash into the possible_victims map
      // to be used in the next iteration
      possible_victims.insert(root.tx_hash, vec![]);
   }

   possible_sandwiches.into_values().collect()
}
```

This function scans the transaction tree for repeated contract (`to`) addresses. It builds [`PossibleSandwich`](https://github.com/SorellaLabs/brontes/blob/5b1d1b4e30d5c92b2a0bc56ff4dd441aed533681/crates/brontes-inspect/src/mev_inspectors/sandwich/types.rs#L7) structs for each potential attack scenario. Here's how:

1. It tracks duplicate contracts.
2. It identifies transactions between duplicates as potential victims.
3. It creates or updates `PossibleSandwich` structs for each scenario.

The `PossibleSandwich` results, identified by duplicate sender & contract, are deduplicated before proceeding to the next step.

### Step 3: Partitioning Possible Sandwiches

Here's how partitioning works:

- We iterate through victim sets in each sandwich.
- Empty victim sets signal a break in the sandwich.
- We create new `PossibleSandwich` structs at these breaks.

<div style="text-align: center;">
 <img src="sandwich/partition-sandwich.png" alt="Possible Sandwich Partitioning" style="border-radius: 20px; width: 600px; height: auto;">
</div>

> **Note:** Our partitioning assumes attackers maximize efficiency. Multiple attacker transactions without intervening victims may lead to unexpected results. If you find examples breaking this assumption, please report them for a bounty.

### Step 4: Analyze Possible Sandwich Attacks

#### Pool Overlap Check

Front-run and back-run transactions must swap on at least one common liquidity pool.

<div style="text-align: center;">
 <img src="sandwich/overlap-check.png" alt="Sandwich Pool Overlap Check" style="border-radius: 20px; width: 600px; height: auto;">
</div>

#### Victim Verification

After confirming pool overlap, we validate interleaved transactions as victims:

1. Group victim transactions by EOA to account for multi-step victim operations (e.g., approval and swap).
2. An EOA is a victim if it:
   - Swaps on the same pool and direction as the front-run
   - Swaps on the same pool and opposite direction as the back-run

<div style="text-align: center;">
 <img src="sandwich/victim-trade-overlap.png" alt="Victim Trade Overlap Check" style="border-radius: 20px; width: 600px; height: auto;">
</div>

A `PossibleSandwich` is confirmed if:

- At least 50% of EOAs are considered victims
- At least one complete sandwich is detected (e.g a victim swap overlaps with both front-run and back-run in pool & direction)

If confirmed, we proceed to Step 5. Otherwise, we initiate recursive verification.

#### Recursive Sandwich Verification

For unconfirmed sandwiches, we employ a recursive strategy to explore all possible transaction combinations:

<div style="text-align: center;">
 <img src="sandwich/recursive-check.png" alt="Recursive Sandwich Split" style="border-radius: 20px; width: 600px; height: auto;">
</div>

1. The process stops after 6 recursive iterations.
2. We apply two types of "shrinking":

   **Back Shrink**:

   - Remove the last victim set
   - Use the last front-run as the new back-run
   - Recalculate the sandwich (run step 4 on the new sandwich)

   **Front Shrink**:

   - Remove the first victim set
   - Remove the first front-run transaction
   - Retain the original back-run
   - Recalculate the sandwich (run step 4 on the new sandwich)

3. We continue this process as long as:
   - There's more than one front-run transaction
   - Victim sets aren't empty
   - At least one victim has non-empty swaps or transfers

### Step 5: Calculate Sandwich PnL

For confirmed sandwiches:

1. Calculate searcher revenue: Balance deltas of searcher addresses & sibling address (e.g piggy bank address) if applicable
2. Calculate searcher cost: Sum of gas costs for all attacker transactions
3. Profit = Revenue - Cost
