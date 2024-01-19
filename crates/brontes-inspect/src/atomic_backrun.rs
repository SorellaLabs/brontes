use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use brontes_database::Metadata;
use brontes_database_libmdbx::Libmdbx;
use brontes_types::{
    classified_mev::{AtomicBackrun, MevType},
    normalized_actions::Actions,
    tree::{BlockTree, GasDetails},
    ToFloatNearest,
};
use itertools::Itertools;
use malachite::{num::basic::traits::Zero, Rational};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use reth_primitives::{Address, B256};

use crate::{shared_utils::SharedInspectorUtils, ClassifiedMev, Inspector, SpecificMev};

pub struct AtomicBackrunInspector<'db> {
    inner: SharedInspectorUtils<'db>,
}

impl<'db> AtomicBackrunInspector<'db> {
    pub fn new(quote: Address, db: &'db Libmdbx) -> Self {
        Self { inner: SharedInspectorUtils::new(quote, db) }
    }
}

#[async_trait::async_trait]
impl Inspector for AtomicBackrunInspector<'_> {
    async fn process_tree(
        &self,
        tree: Arc<BlockTree<Actions>>,
        meta_data: Arc<Metadata>,
    ) -> Vec<(ClassifiedMev, Box<dyn SpecificMev>)> {
        let intersting_state = tree.collect_all(|node| {
            (
                node.data.is_swap() || node.data.is_transfer(),
                node.subactions
                    .iter()
                    .any(|action| action.is_swap() || action.is_transfer()),
            )
        });

        intersting_state
            .into_par_iter()
            .filter_map(|(tx, swaps)| {
                let gas_details = tree.get_gas_details(tx)?.clone();
                let root = tree.get_root(tx)?;
                let idx = root.get_block_position();

                // take all swaps and remove swaps that don't do more than a single swap. This
                // removes all cex <> dex arbs and one off funky swaps
                let mut tokens: HashMap<Address, Vec<Address>> = HashMap::new();
                swaps
                    .iter()
                    .filter(|s| s.is_swap())
                    .map(|f| f.clone().force_swap())
                    .for_each(|swap| {
                        let e = tokens.entry(swap.pool).or_default();
                        e.push(swap.token_in);
                        e.push(swap.token_out);
                    });

                let entries = tokens.len() * 2;
                let overlaps = tokens
                    .values()
                    .flatten()
                    .sorted()
                    .dedup_with_count()
                    .map(|(i, _)| i)
                    .sum::<usize>();

                if overlaps <= entries {
                    return None
                }

                self.process_swaps(
                    tx,
                    idx,
                    root.head.address,
                    root.head.data.get_to_address(),
                    meta_data.clone(),
                    gas_details,
                    vec![swaps],
                )
            })
            .collect::<Vec<_>>()
    }
}

impl AtomicBackrunInspector<'_> {
    fn process_swaps(
        &self,
        tx_hash: B256,
        idx: usize,
        eoa: Address,
        mev_contract: Address,
        metadata: Arc<Metadata>,
        gas_details: GasDetails,
        searcher_actions: Vec<Vec<Actions>>,
    ) -> Option<(ClassifiedMev, Box<dyn SpecificMev>)> {
        let deltas = self.inner.calculate_token_deltas(&searcher_actions);

        let addr_usd_deltas =
            self.inner
                .usd_delta_by_address(idx, deltas, metadata.clone(), false)?;

        let mev_profit_collector = self.inner.profit_collectors(&addr_usd_deltas);

        let rev_usd = addr_usd_deltas
            .values()
            .fold(Rational::ZERO, |acc, delta| acc + delta);

        let gas_used = gas_details.gas_paid();
        let gas_used_usd = metadata.get_gas_price_usd(gas_used);

        let unique_tokens = searcher_actions
            .iter()
            .flatten()
            .filter(|f| f.is_swap())
            .map(|f| f.force_swap_ref())
            .flat_map(|s| vec![s.token_in, s.token_out])
            .collect::<HashSet<_>>();

        // most likely just a false positive unless the person is holding shit_coin
        // inventory.
        // to keep the degens, we don't remove if there is a coinbase.transfer
        //
        // False positives come from this due to there being a small opportunity that
        // exists within a single swap that can only be executed if you hold
        // inventory. Because of this 99% of the time it is normal users who
        // trigger this.
        if unique_tokens.len() == 2 && gas_details.coinbase_transfer.is_none() {
            return None
        }

        // Can change this later to check if people are subsidising arbs to kill ops for
        // competitors
        if &rev_usd - &gas_used_usd <= Rational::ZERO {
            return None
        }

        let classified = ClassifiedMev {
            mev_type: MevType::Backrun,
            tx_hash,
            mev_contract,
            block_number: metadata.block_num,
            mev_profit_collector,
            eoa,
            finalized_bribe_usd: gas_used_usd.clone().to_float(),
            finalized_profit_usd: (rev_usd - gas_used_usd).to_float(),
        };

        let swaps = searcher_actions
            .into_iter()
            .flatten()
            .filter(|actions| actions.is_swap())
            .map(|s| s.force_swap())
            .collect::<Vec<_>>();

        let backrun = Box::new(AtomicBackrun { tx_hash, gas_details, swaps });

        Some((classified, backrun))
    }
}

#[cfg(test)]
mod tests {
    use std::{env, str::FromStr, time::SystemTime};

    use brontes_classifier::Classifier;
    use brontes_database::clickhouse::Clickhouse;
    use brontes_database_libmdbx::Libmdbx;
    use serial_test::serial;
    use tokio::sync::mpsc::unbounded_channel;

    use super::*;
    use crate::test_utils::{InspectorTestUtils, InspectorTxRunConfig, USDC_ADDRESS};

    #[tokio::test]
    #[serial]
    async fn test_backrun() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 0.1);

        let tx = hex!("76971a4f00a0a836322c9825b6edf06c8c49bf4261ef86fc88893154283a7124").into();
        let config = InspectorTxRunConfig::new(MevType::Backrun)
            .with_mev_tx_hashes(vec![tx])
            .with_dex_prices()
            .with_expected_profit_usd(0.188588)
            .with_expected_gas_used(71.632668);

        inspector_util.run_inspector(config, None).await.unwrap();
    }
}
