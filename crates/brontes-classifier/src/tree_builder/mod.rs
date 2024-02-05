use std::{cmp::min, sync::Arc};

use brontes_types::ToScaledRational;
mod tree_pruning;
mod utils;
use brontes_core::missing_token_info::load_missing_token_info;
use brontes_database::libmdbx::{LibmdbxReader, LibmdbxWriter};
use brontes_pricing::types::DexPriceMsg;
use brontes_types::{
    normalized_actions::{Actions, NormalizedAction, SelfdestructWithIndex},
    structured_trace::{TraceActions, TransactionTraceWithLogs, TxTrace},
    traits::TracingProvider,
    tree::{BlockTree, GasDetails, Node, Root},
};
use futures::future::join_all;
use itertools::Itertools;
use malachite::num::arithmetic::traits::Abs;
use reth_primitives::{Address, Header};
use reth_rpc_types::trace::parity::Action;
use tokio::sync::mpsc::UnboundedSender;
use tracing::{debug, error};
use tree_pruning::{
    account_for_tax_tokens, remove_collect_transfers, remove_mint_transfers, remove_swap_transfers,
};
use utils::{decode_transfer, get_coinbase_transfer};

use self::transfer::try_decode_transfer;
use crate::{
    classifiers::{DiscoveryProtocols, *},
    ActionCollection, FactoryDecoderDispatch,
};

//TODO: Document this module
#[derive(Debug, Clone)]
pub struct Classifier<'db, T: TracingProvider, DB: LibmdbxReader + LibmdbxWriter> {
    libmdbx:               &'db DB,
    provider:              Arc<T>,
    pricing_update_sender: UnboundedSender<DexPriceMsg>,
}

impl<'db, T: TracingProvider, DB: LibmdbxReader + LibmdbxWriter> Classifier<'db, T, DB> {
    pub fn new(
        libmdbx: &'db DB,
        pricing_update_sender: UnboundedSender<DexPriceMsg>,
        provider: Arc<T>,
    ) -> Self {
        Self { libmdbx, pricing_update_sender, provider }
    }

    pub async fn build_block_tree(
        &self,
        traces: Vec<TxTrace>,
        header: Header,
    ) -> BlockTree<Actions> {
        let tx_roots = self.build_all_tx_trees(traces, &header).await;
        let mut tree = BlockTree::new(header, tx_roots.len());

        // send out all updates
        let further_classification_requests = self.process_tx_roots(tx_roots, &mut tree);

        Self::prune_tree(&mut tree);
        finish_classification(&mut tree, further_classification_requests);

        tree.finalize_tree();

        tree
    }

    fn process_tx_roots(
        &self,
        tx_roots: Vec<TxTreeResult>,
        tree: &mut BlockTree<Actions>,
    ) -> Vec<Option<(usize, Vec<u64>)>> {
        let further_classification_requests = tx_roots
            .into_iter()
            .map(|root_data| {
                tree.insert_root(root_data.root);
                root_data.pool_updates.into_iter().for_each(|update| {
                    self.pricing_update_sender.send(update).unwrap();
                });
                root_data.further_classification_requests
            })
            .collect_vec();

        further_classification_requests
    }

    pub(crate) fn prune_tree(tree: &mut BlockTree<Actions>) {
        // tax token accounting should always be first.
        account_for_tax_tokens(tree);
        remove_swap_transfers(tree);
        remove_mint_transfers(tree);
        remove_collect_transfers(tree);
    }

    pub(crate) async fn build_all_tx_trees(
        &self,
        traces: Vec<TxTrace>,
        header: &Header,
    ) -> Vec<TxTreeResult> {
        join_all(
            traces
                .into_iter()
                .enumerate()
                .map(|(tx_idx, mut trace)| async move {
                    // here only traces where the root tx failed are filtered out
                    if trace.trace.is_empty() || !trace.is_success {
                        return None
                    }
                    // post classification processing collectors
                    let mut further_classification_requests = Vec::new();
                    let mut pool_updates: Vec<DexPriceMsg> = Vec::new();

                    let root_trace = trace.trace.remove(0);
                    let address = root_trace.get_from_addr();
                    let classification = self
                        .process_classification(
                            header.number,
                            None,
                            tx_idx as u64,
                            0,
                            root_trace,
                            &mut further_classification_requests,
                            &mut pool_updates,
                        )
                        .await;

                    let node = Node::new(0, address, classification, vec![]);

                    let mut tx_root = Root {
                        position:    tx_idx,
                        head:        node,
                        tx_hash:     trace.tx_hash,
                        private:     false,
                        gas_details: GasDetails {
                            coinbase_transfer:   None,
                            gas_used:            trace.gas_used,
                            effective_gas_price: trace.effective_price,
                            priority_fee:        trace.effective_price
                                - (header.base_fee_per_gas.unwrap() as u128),
                        },
                    };

                    for (index, trace) in trace.trace.into_iter().enumerate() {
                        if let Some(coinbase) = &mut tx_root.gas_details.coinbase_transfer {
                            *coinbase +=
                                get_coinbase_transfer(header.beneficiary, &trace.trace.action)
                                    .unwrap_or_default()
                        } else {
                            tx_root.gas_details.coinbase_transfer =
                                get_coinbase_transfer(header.beneficiary, &trace.trace.action);
                        }

                        let classification = self
                            .process_classification(
                                header.number,
                                Some(&tx_root.head),
                                tx_idx as u64,
                                (index + 1) as u64,
                                trace.clone(),
                                &mut further_classification_requests,
                                &mut pool_updates,
                            )
                            .await;

                        let from_addr = trace.get_from_addr();

                        let node = Node::new(
                            (index + 1) as u64,
                            from_addr,
                            classification,
                            trace.trace.trace_address,
                        );

                        tx_root.insert(node);
                    }

                    // Here we reverse the requests to ensure that we always classify the most
                    // nested action & its children first. This is to prevent the
                    // case where we classify a parent action where its children also require
                    // further classification
                    let tx_classification_requests = if !further_classification_requests.is_empty()
                    {
                        further_classification_requests.reverse();
                        Some((tx_idx, further_classification_requests))
                    } else {
                        None
                    };
                    Some(TxTreeResult {
                        root: tx_root,
                        further_classification_requests: tx_classification_requests,
                        pool_updates,
                    })
                }),
        )
        .await
        .into_iter()
        .filter_map(|f| f)
        .collect_vec()
    }

    async fn process_classification(
        &self,
        block_number: u64,
        root_head: Option<&Node<Actions>>,
        tx_index: u64,
        trace_index: u64,
        trace: TransactionTraceWithLogs,
        further_classification_requests: &mut Vec<u64>,
        pool_updates: &mut Vec<DexPriceMsg>,
    ) -> Actions {
        let (update, classification) = self
            .classify_node(block_number, root_head, tx_index as u64, trace, trace_index)
            .await;

        // Here we are marking more complex actions that require data
        // that can only be retrieved by classifying it's action and
        // all subsequent child actions.
        if classification.continue_classification() {
            further_classification_requests.push(classification.get_trace_index());
        }

        if let Actions::Transfer(transfer) = &classification {
            if self
                .libmdbx
                .try_get_token_info(transfer.token.address)
                .unwrap()
                .is_none()
            {
                load_missing_token_info(
                    &self.provider,
                    self.libmdbx,
                    block_number,
                    transfer.token.address,
                )
                .await;
            }
        }

        // if we have a discovered pool, check if its new
        update.into_iter().for_each(|update| {
            match update {
                DexPriceMsg::DiscoveredPool(pool, block) => {
                    if !self.contains_pool(pool.pool_address).unwrap() {
                        self.pricing_update_sender
                            .send(DexPriceMsg::DiscoveredPool(pool.clone(), block))
                            .unwrap();

                        if self
                            .libmdbx
                            .insert_pool(
                                block_number,
                                pool.pool_address,
                                [pool.tokens[0], pool.tokens[1]],
                                pool.protocol,
                            )
                            .is_err()
                        {
                            error!("failed to insert discovered pool into libmdbx");
                        }
                    }
                }
                rest => {
                    pool_updates.push(rest);
                }
            };
        });

        classification
    }

    fn contains_pool(&self, address: Address) -> eyre::Result<bool> {
        Ok(self.libmdbx.get_protocol(address)?.is_some())
    }

    async fn classify_node(
        &self,
        block: u64,
        root_head: Option<&Node<Actions>>,
        tx_idx: u64,
        trace: TransactionTraceWithLogs,
        trace_index: u64,
    ) -> (Vec<DexPriceMsg>, Actions) {
        if trace.trace.error.is_some() {
            return (vec![], Actions::Revert)
        }
        match trace.action_type() {
            Action::Call(_) => return self.classify_call(block, tx_idx, trace, trace_index).await,
            Action::Create(_) => {
                return self
                    .classify_create(block, root_head, tx_idx, trace, trace_index)
                    .await
            }
            Action::Selfdestruct(sd) => {
                return (vec![], Actions::SelfDestruct(SelfdestructWithIndex::new(trace_index, *sd)))
            }
            Action::Reward(_) => return (vec![], Actions::Unclassified(trace)),
        };
    }

    async fn classify_call(
        &self,
        block: u64,
        tx_idx: u64,
        trace: TransactionTraceWithLogs,
        trace_index: u64,
    ) -> (Vec<DexPriceMsg>, Actions) {
        if trace.is_static_call() {
            return (vec![], Actions::Unclassified(trace))
        }
        let from_address = trace.get_from_addr();
        let target_address = trace.get_to_address();

        let call_data = trace.get_calldata();
        let return_bytes = trace.get_return_calldata();

        if let Some(results) = ProtocolClassifications::default().dispatch(
            trace_index,
            call_data,
            return_bytes.clone(),
            from_address,
            target_address,
            trace.msg_sender,
            &trace.logs,
            self.libmdbx,
            block,
            tx_idx,
        ) {
            return (vec![DexPriceMsg::Update(results.0)], results.1)
        } else if let Some(mut transfer) = try_decode_transfer(
            tx_idx,
            trace.get_calldata(),
            trace.get_from_addr(),
            {
                if trace.is_delegate_call() {
                    // if we got delegate, the actual token address
                    // is the from addr (proxy) for pool swaps. without
                    // this our math gets fucked
                    trace.get_from_addr()
                } else {
                    trace.get_to_address()
                }
            },
            self.libmdbx,
        ) {
            // go through the log to look for descrepency of transfer amount
            for log in &trace.logs {
                if let Some((addr, from, to, amount)) = decode_transfer(log) {
                    if addr != transfer.token.address || transfer.from != from || transfer.to != to
                    {

                        continue
                    }

                    let addr = if trace.is_delegate_call() {
                        // if we got delegate, the actual token address
                        // is the from addr (proxy) for pool swaps. without
                        // this our math gets fucked
                        trace.get_from_addr()
                    } else {
                        addr
                    };

                    if self.libmdbx.try_get_token_info(addr).unwrap().is_none() {
                        load_missing_token_info(&self.provider, self.libmdbx, block, addr).await;
                    }
                    let decimals = transfer.token.decimals;
                    let log_am = amount.to_scaled_rational(decimals);

                    if log_am != transfer.amount {
                        let transferred_amount = min(&log_am, &transfer.amount).clone();
                        let fee = (&log_am - &transfer.amount).abs();
                        transfer.amount = transferred_amount;
                        transfer.fee = fee;

                        tracing::info!(?transfer, "tax token");
                    }
                    break;
                }
            }

            return (vec![], Actions::Transfer(transfer))
        }
        (vec![], Actions::Unclassified(trace))
    }

    async fn classify_create(
        &self,
        block: u64,
        root_head: Option<&Node<Actions>>,
        tx_idx: u64,
        trace: TransactionTraceWithLogs,
        trace_index: u64,
    ) -> (Vec<DexPriceMsg>, Actions) {
        let from_address = trace.get_from_addr();
        let created_addr = trace.get_create_output();

        // get the immediate parent node of this create action so that we can decode the
        // deployment function params
        let node_data = match root_head {
            Some(head) => head.get_immediate_parent_node(trace_index - 1),
            None => return (vec![], Actions::Unclassified(trace)),
        };
        let Some(node_data) = node_data else {
            debug!(block, tx_idx, "failed to find create parent node");
            return (vec![], Actions::Unclassified(trace));
        };

        let Some(calldata) = node_data.data.get_calldata() else {
            return (vec![], Actions::Unclassified(trace));
        };

        return (
            DiscoveryProtocols::default()
                .dispatch(self.provider.clone(), from_address, created_addr, calldata)
                .await
                .into_iter()
                .map(|pool| DexPriceMsg::DiscoveredPool(pool, block))
                .collect::<Vec<_>>(),
            Actions::Unclassified(trace),
        )
    }

    pub fn close(&self) {
        self.pricing_update_sender
            .send(DexPriceMsg::Closed)
            .unwrap();
    }
}

/// This function is used to finalize the classification of complex actions
/// that contain nested sub-actions that are required to finalize the higher
/// level classification (e.g: flashloan actions)
fn finish_classification(
    tree: &mut BlockTree<Actions>,
    further_classification_requests: Vec<Option<(usize, Vec<u64>)>>,
) {
    tree.collect_and_classify(&further_classification_requests)
}

pub struct TxTreeResult {
    pub pool_updates: Vec<DexPriceMsg>,
    pub further_classification_requests: Option<(usize, Vec<u64>)>,
    pub root: Root<Actions>,
}

#[cfg(test)]
pub mod test {
    use std::collections::{HashMap, HashSet};

    use alloy_primitives::{hex, Address, B256, U256};
    use brontes_types::{
        db::token_info::TokenInfoWithAddress,
        normalized_actions::{Actions, NormalizedLiquidation},
        Protocol,
    };
    use malachite::Rational;
    use serial_test::serial;

    use crate::test_utils::ClassifierTestUtils;

    #[tokio::test]
    #[serial]
    async fn test_remove_swap_transfer() {
        let classifier_utils = ClassifierTestUtils::new();
        let jared_tx =
            B256::from(hex!("d40905a150eb45f04d11c05b5dd820af1b381b6807ca196028966f5a3ba94b8d"));

        let tree = classifier_utils.build_raw_tree_tx(jared_tx).await.unwrap();

        let swap = tree.collect(jared_tx, |node| {
            (
                node.data.is_swap() || node.data.is_transfer(),
                node.subactions
                    .iter()
                    .any(|action| action.is_swap() || action.is_transfer()),
            )
        });
        let mut swaps: HashMap<TokenInfoWithAddress, HashSet<Rational>> = HashMap::default();

        for i in &swap {
            if let Actions::Swap(s) = i {
                swaps
                    .entry(s.token_in.clone())
                    .or_default()
                    .insert(s.amount_in.clone());
                swaps
                    .entry(s.token_out.clone())
                    .or_default()
                    .insert(s.amount_out.clone());
            }
        }

        for i in &swap {
            if let Actions::Transfer(t) = i {
                if swaps.get(&t.token).map(|i| i.contains(&t.amount)) == Some(true) {
                    assert!(false, "found a transfer that was part of a swap");
                }
            }
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_aave_v3_liquidation() {
        let classifier_utils = ClassifierTestUtils::new();
        let aave_v3_liquidation =
            B256::from(hex!("dd951e0fc5dc4c98b8daaccdb750ff3dc9ad24a7f689aad2a088757266ab1d55"));

        let eq_action = Actions::Liquidation(NormalizedLiquidation {
            protocol:              Protocol::AaveV3,
            liquidated_collateral: Rational::from_signeds(165516722, 100000000),
            covered_debt:          Rational::from_signeds(63857746423_i64, 1000000),
            debtor:                Address::from(hex!("e967954b9b48cb1a0079d76466e82c4d52a8f5d3")),
            debt_asset:            classifier_utils
                .get_token_info(Address::from(hex!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"))),
            collateral_asset:      classifier_utils
                .get_token_info(Address::from(hex!("2260fac5e5542a773aa44fbcfedf7c193bc2c599"))),
            liquidator:            Address::from(hex!("80d4230c0a68fc59cb264329d3a717fcaa472a13")),
            pool:                  Address::from(hex!("5faab9e1adbddad0a08734be8a52185fd6558e14")),
            trace_index:           6,
        });

        classifier_utils
            .contains_action(aave_v3_liquidation, 0, eq_action, Actions::liquidation_collect_fn())
            .await
            .unwrap();
    }
}
