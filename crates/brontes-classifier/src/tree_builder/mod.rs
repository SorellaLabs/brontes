use std::sync::Arc;
mod tree_pruning;
mod utils;
use brontes_database::libmdbx::{
    tables::{AddressToProtocol, AddressToTokens, PoolCreationBlocks, TokenDecimals},
    types::{
        address_to_protocol::AddressToProtocolData, address_to_tokens::AddressToTokensData,
        pool_creation_block::PoolCreationBlocksData,
    },
    Libmdbx,
};
use brontes_pricing::types::DexPriceMsg;
use brontes_types::{
    db::{address_to_tokens::PoolTokens, pool_creation_block::PoolsToAddresses},
    exchanges::Protocol,
    extra_processing::ExtraProcessing,
    normalized_actions::{Actions, NormalizedAction, NormalizedTransfer, SelfdestructWithIndex},
    structured_trace::{TraceActions, TransactionTraceWithLogs, TxTrace},
    traits::TracingProvider,
    tree::{BlockTree, GasDetails, Node, Root},
};
use futures::future::join_all;
use itertools::Itertools;
use reth_primitives::{Address, Header};
use reth_rpc_types::trace::parity::Action;
use tokio::sync::mpsc::UnboundedSender;
use tracing::error;
use tree_pruning::{
    account_for_tax_tokens, remove_collect_transfers, remove_mint_transfers, remove_swap_transfers,
};
use utils::{decode_transfer, get_coinbase_transfer};

use crate::{
    classifiers::{DiscoveryProtocols, *},
    ActionCollection, FactoryDecoderDispatch,
};

//TODO: Document this module
#[derive(Debug, Clone)]
pub struct Classifier<'db, T: TracingProvider> {
    libmdbx:               &'db Libmdbx,
    provider:              Arc<T>,
    pricing_update_sender: UnboundedSender<DexPriceMsg>,
}

impl<'db, T: TracingProvider> Classifier<'db, T> {
    pub fn new(
        libmdbx: &'db Libmdbx,
        pricing_update_sender: UnboundedSender<DexPriceMsg>,
        provider: Arc<T>,
    ) -> Self {
        Self { libmdbx, pricing_update_sender, provider }
    }

    pub async fn build_block_tree(
        &self,
        traces: Vec<TxTrace>,
        header: Header,
    ) -> (ExtraProcessing, BlockTree<Actions>) {
        let tx_roots = self.build_all_tx_trees(traces, &header).await;
        let mut tree = BlockTree::new(header, tx_roots.len());

        // send out all updates
        let (further_classification_requests, missing_data_requests) =
            self.process_tx_roots(tx_roots, &mut tree);

        Self::prune_tree(&mut tree);
        finish_classification(&mut tree, further_classification_requests);

        tree.finalize_tree();

        let mut addresses_missing_decimals = missing_data_requests
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();

        // need to sort before we can dedup
        addresses_missing_decimals.sort();
        addresses_missing_decimals.dedup();

        let processing = ExtraProcessing { tokens_decimal_fill: addresses_missing_decimals };

        (processing, tree)
    }

    fn process_tx_roots(
        &self,
        tx_roots: Vec<TxTreeResult>,
        tree: &mut BlockTree<Actions>,
    ) -> (Vec<Option<(usize, Vec<u64>)>>, Vec<Vec<Address>>) {
        let (further_classification_requests, missing_data_requests): (Vec<_>, Vec<_>) = tx_roots
            .into_iter()
            .map(|root_data| {
                tree.insert_root(root_data.root);
                root_data.pool_updates.into_iter().for_each(|update| {
                    self.pricing_update_sender.send(update).unwrap();
                });
                (root_data.further_classification_requests, root_data.missing_data_requests)
            })
            .unzip();

        (further_classification_requests, missing_data_requests)
    }

    pub(crate) fn prune_tree(tree: &mut BlockTree<Actions>) {
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
                    let mut missing_decimals = Vec::new();
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
                            &mut missing_decimals,
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
                                &mut missing_decimals,
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
                        missing_data_requests: missing_decimals,
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
        missing_decimals: &mut Vec<Address>,
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
            if self.try_get_decimals(&transfer.token).unwrap().is_none() {
                missing_decimals.push(transfer.token);
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

    fn insert_pool(
        &self,
        block: u64,
        address: Address,
        tokens: [Address; 2],
        classifier_name: Protocol,
    ) -> eyre::Result<()> {
        self.libmdbx
            .write_table::<AddressToProtocol, AddressToProtocolData>(&vec![
                AddressToProtocolData { address, classifier_name },
            ])?;

        let tx = self.libmdbx.ro_tx()?;
        let mut addrs = tx
            .get::<PoolCreationBlocks>(block)?
            .map(|i| i.0)
            .unwrap_or(vec![]);

        addrs.push(address);
        self.libmdbx
            .write_table::<PoolCreationBlocks, PoolCreationBlocksData>(&vec![
                PoolCreationBlocksData {
                    block_number: block,
                    pools:        PoolsToAddresses(addrs),
                },
            ])?;

        self.libmdbx
            .write_table::<AddressToTokens, AddressToTokensData>(&vec![AddressToTokensData {
                address,
                tokens: PoolTokens {
                    token0: tokens[0],
                    token1: tokens[1],
                    init_block: block,
                    ..Default::default()
                },
            }])?;

        Ok(())
    }

    fn contains_pool(&self, address: Address) -> eyre::Result<bool> {
        let tx = self.libmdbx.ro_tx()?;
        Ok(tx.get::<AddressToProtocol>(address)?.is_some())
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
            Action::Call(_) => return self.classify_call(block, tx_idx, trace, trace_index),
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

    fn try_get_classifier(&self, target_address: Address) -> Option<Box<dyn ActionCollection>> {
        let db_tx = self.libmdbx.ro_tx().ok()?;

        let protocol = db_tx.get::<AddressToProtocol>(target_address).ok()??;
        match protocol {
            Protocol::UniswapV2 => Some(Box::new(UniswapV2Classifier::default())),
            Protocol::SushiSwapV2 => Some(Box::new(SushiSwapV2Classifier::default())),
            Protocol::UniswapV3 => Some(Box::new(UniswapV3Classifier::default())),
            Protocol::SushiSwapV3 => Some(Box::new(SushiSwapV3Classifier::default())),
            Protocol::CurveCryptoSwap => {
                Some(Box::new(CurveCryptoSwapClassifier::default()))
            }
            Protocol::AaveV2 => Some(Box::new(AaveV2Classifier::default())),
            Protocol::AaveV3 => Some(Box::new(AaveV3Classifier::default())),
            Protocol::UniswapX => Some(Box::new(UniswapXClassifier::default())),
            protocol => {
                error!("no classifier for {:?}, consider building one", protocol);
                None
            }
        }
    }

    fn classify_call(
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

        if let Some(classifier) = self.try_get_classifier(target_address) {
            let db_tx = self.libmdbx.ro_tx().unwrap();

            let call_data = trace.get_calldata();
            let return_bytes = trace.get_return_calldata();

            let results = classifier.dispatch(
                trace_index,
                call_data,
                return_bytes.clone(),
                from_address,
                target_address,
                trace.msg_sender,
                &trace.logs,
                &db_tx,
                block,
                tx_idx,
            );

            if let Some(decoded_result) = results {
                return (vec![DexPriceMsg::Update(decoded_result.0)], decoded_result.1)
            }
        } else if trace.logs.len() > 0 {
            // A transfer should always be in its own call trace and have 1 log.
            // if forever reason there is a case with multiple logs, we take the first
            // transfer
            for log in &trace.logs {
                if let Some((addr, from, to, value)) = decode_transfer(log) {
                    let addr = if trace.is_delegate_call() {
                        // if we got delegate, the actual token address
                        // is the from addr (proxy) for pool swaps. without
                        // this our math gets fucked
                        trace.get_from_addr()
                    } else {
                        addr
                    };

                    return (
                        vec![],
                        Actions::Transfer(NormalizedTransfer {
                            trace_index,
                            to,
                            from,
                            token: addr,
                            amount: value,
                        }),
                    )
                }
            }
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
            Some(head) => head.get_most_recent_parent_node(trace_index - 1),
            None => return (vec![], Actions::Unclassified(trace)),
        };
        let Some(node_data) = node_data else {
            error!(block, tx_idx, "failed to find create parent node");
            return (vec![], Actions::Unclassified(trace));
        };

        let Some(calldata) = node_data.data.get_calldata() else {
            return (vec![], Actions::Unclassified(trace));
        };

        return (
            DiscoveryProtocols::dispatch(
                self.provider.clone(),
                from_address,
                created_addr,
                calldata,
            )
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

    pub fn try_get_decimals(&self, token_addr: &Address) -> eyre::Result<Option<u8>> {
        let tx = self.libmdbx.ro_tx()?;
        Ok(tx.get::<TokenDecimals>(*token_addr)?)
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
    pub missing_data_requests: Vec<Address>,
    pub pool_updates: Vec<DexPriceMsg>,
    pub further_classification_requests: Option<(usize, Vec<u64>)>,
    pub root: Root<Actions>,
}

#[cfg(test)]
pub mod test {
    use std::{
        collections::{HashMap, HashSet},
        env,
    };

    use alloy_primitives::{hex, B256, U256};
    use brontes_pricing::types::DexPriceMsg;
    use brontes_types::normalized_actions::{Actions, NormalizedLiquidation};
    use reth_primitives::Address;
    use serial_test::serial;

    use crate::test_utils::ClassifierTestUtils;

    #[tokio::test]
    #[serial]
    async fn test_remove_swap_transfer() {
        let block_num = 18530326;
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
        let mut swaps: HashMap<Address, HashSet<U256>> = HashMap::default();

        for i in &swap {
            if let Actions::Swap(s) = i {
                swaps.entry(s.token_in).or_default().insert(s.amount_in);
                swaps.entry(s.token_out).or_default().insert(s.amount_out);
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
            liquidated_collateral: U256::from(165516722u64),
            covered_debt:          U256::from(63857746423u64),
            debtor:                Address::from(hex!("e967954b9b48cb1a0079d76466e82c4d52a8f5d3")),
            debt_asset:            Address::from(hex!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")),
            collateral_asset:      Address::from(hex!("2260fac5e5542a773aa44fbcfedf7c193bc2c599")),
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
