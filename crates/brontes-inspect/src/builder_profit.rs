use std::sync::Arc;

use alloy_primitives::Address;
use brontes_database::libmdbx::LibmdbxReader;
use brontes_types::{mev::Bundle, normalized_actions::Actions, tree::BlockTree, TreeSearchArgs};
use malachite::Rational;

use crate::{Inspector, Metadata};
pub struct BuilderProfitInspector<DB: LibmdbxReader> {
    inner: &'static DB,
}

impl<DB: LibmdbxReader> BuilderProfitInspector<DB> {
    pub fn new(db: &'static DB) -> Self {
        Self { inner: db }
    }

    pub fn calculate_builder_profit(
        &self,
        builder_address: Address,
        tree: Arc<BlockTree<Actions>>,
    ) -> Result<u128, Box<dyn std::error::Error + Send + Sync>> {
        let coinbase_transfers = tree
            .tx_roots
            .iter()
            .filter_map(|root| root.gas_details.coinbase_transfer)
            .sum::<u128>(); // Specify the type of sum

        let builder_collateral_amount = self
            .inner
            .try_fetch_builder_info(builder_address)
            .map(|builder_info| {
                tree.collect_all(|node| TreeSearchArgs {
                    collect_current_node:  node.data.get_from_address()
                        == builder_info.ultrasound_relay_collateral_address.unwrap()
                        && node.data.is_eth_transfer(),
                    child_node_to_collect: node.get_all_sub_actions().iter().any(|sub_node| {
                        sub_node.get_from_address()
                            == builder_info.ultrasound_relay_collateral_address.unwrap()
                            && sub_node.is_eth_transfer()
                    }),
                })
                .iter()
                .flat_map(|(_fixed_bytes, actions)| {
                    actions.iter().filter_map(|action| {
                        if let Actions::Transfer(transfer) = action {
                            Some(transfer.amount.clone())
                        } else {
                            None
                        }
                    })
                })
                .map(|rational| u128::try_from(&Rational::from(rational)))
                .filter_map(Result::ok)
                .sum::<u128>()
            })
            .unwrap_or_default(); // Handle the case when try_fetch_builder_info returns None

        Ok(coinbase_transfers - builder_collateral_amount)
    }
}

#[async_trait::async_trait]
impl<DB: LibmdbxReader> Inspector for BuilderProfitInspector<DB> {
    async fn process_tree(
        &self,
        tree: Arc<BlockTree<Actions>>,
        metadata: Arc<Metadata>,
    ) -> Vec<Bundle> {
        Vec::new()
    }
}
