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
                    tree.avg_priority_fee,
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
        avg_priority_fee: u128,
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
        // to keep the degens, if they coinbase.transfer or send 3x the average priority
        // fee, we include them.
        //
        // False positives come from this due to there being a small opportunity that
        // exists within a single swap that can only be executed if you hold
        // inventory. Because of this 99% of the time it is normal users who
        // trigger this.
        if unique_tokens.len() == 2
            && gas_details.coinbase_transfer.is_none()
            && avg_priority_fee * 2 > gas_details.priority_fee
        {
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

        let backrun = Box::new(AtomicBackrun {
            tx_hash,
            gas_details,
            swaps_index: swaps.iter().map(|s| s.trace_index).collect::<Vec<_>>(),
            swaps_from: swaps.iter().map(|s| s.from).collect::<Vec<_>>(),
            swaps_pool: swaps.iter().map(|s| s.pool).collect::<Vec<_>>(),
            swaps_token_in: swaps.iter().map(|s| s.token_in).collect::<Vec<_>>(),
            swaps_token_out: swaps.iter().map(|s| s.token_out).collect::<Vec<_>>(),
            swaps_amount_in: swaps.iter().map(|s| s.amount_in.to()).collect::<Vec<_>>(),
            swaps_amount_out: swaps.iter().map(|s| s.amount_out.to()).collect::<Vec<_>>(),
        });

        Some((classified, backrun))
    }
}

#[cfg(test)]
mod tests {
    use std::{env, str::FromStr, time::SystemTime};

    use brontes_classifier::Classifier;
    use brontes_core::{init_tracing, test_utils::init_trace_parser};
    use brontes_database::clickhouse::Clickhouse;
    use brontes_database_libmdbx::Libmdbx;
    use serial_test::serial;
    use tokio::sync::mpsc::unbounded_channel;

    use super::*;

    #[tokio::test]
    #[serial]
    async fn test_backrun() {
        dotenv::dotenv().ok();
        init_tracing();
        let block_num = 18522278;
        let brontes_db_endpoint = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");
        let libmdbx = Libmdbx::init_db(brontes_db_endpoint, None).unwrap();
        let (tx, _rx) = unbounded_channel();

        let tracer = init_trace_parser(tokio::runtime::Handle::current().clone(), tx, &libmdbx);
        let db = Clickhouse::default();

        let classifier = Classifier::new(&libmdbx);

        let block = tracer.execute_block(block_num).await.unwrap();
        let metadata = db.get_metadata(block_num).await;

        let tx = block.0.clone().into_iter().take(60).collect::<Vec<_>>();
        let (missing_token_decimals, tree) = classifier.build_block_tree(tx, block.1);
        let tree = Arc::new(tree);

        let USDC = Address::from_str("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48").unwrap();

        let inspector = Box::new(AtomicBackrunInspector::new(USDC)) as Box<dyn Inspector>;

        let t0 = SystemTime::now();
        let mev = inspector.process_tree(tree.clone(), metadata.into()).await;
        let t1 = SystemTime::now();
        let delta = t1.duration_since(t0).unwrap().as_micros();
        println!("backrun inspector took: {} us", delta);

        // assert!(
        //     mev[0].0.tx_hash
        //         == B256::from_str(

        println!("{:#?}", mev);
    }
}
