use std::{
    cmp::max,
    pin::Pin,
    str::FromStr,
    task::{Context, Poll},
};

use alloy_primitives::Address;
use brontes_classifier::Classifier;
use brontes_core::{
    decoding::{Parser, TracingProvider},
    missing_decimals::MissingDecimals,
};
use brontes_database::libmdbx::{
    tables::{CexPrice, DexPrice, Metadata as MetadataTable, MevBlocks},
    types::{dex_price::make_filter_key_range, mev_block::MevBlocksData},
    Libmdbx,
};
use brontes_inspect::{
    composer::{Composer, ComposerResults},
    Inspector,
};
use brontes_types::{
    classified_mev::{ClassifiedMev, MevBlock, SpecificMev},
    constants::{USDC_ADDRESS, USDT_ADDRESS, WETH_ADDRESS},
    db::{
        cex::{CexPriceMap, CexQuote},
        dex::DexQuotes,
        metadata::{MetadataCombined, MetadataInner, MetadataNoDex},
        mev_block::MevBlockWithClassified,
    },
    extra_processing::Pair,
    normalized_actions::Actions,
    tree::BlockTree,
};
use futures::{Future, FutureExt};
use tracing::{debug, error, info, trace};
type CollectionFut<'a> =
    Pin<Box<dyn Future<Output = (MetadataCombined, BlockTree<Actions>)> + Send + 'a>>;

pub struct BlockInspector<'inspector, T: TracingProvider> {
    block_number: u64,

    parser:            &'inspector Parser<'inspector, T>,
    classifier:        &'inspector Classifier<'inspector, T>,
    database:          &'inspector Libmdbx,
    inspectors:        &'inspector [&'inspector Box<dyn Inspector>],
    composer_future:   Option<Pin<Box<dyn Future<Output = ComposerResults> + Send + 'inspector>>>,
    // pending future data
    classifier_future: Option<CollectionFut<'inspector>>,
    // pending insertion data
    // insertion_future:  Option<Pin<Box<dyn Future<Output = ()> + Send + Sync + 'inspector>>>,
}

impl<'inspector, T: TracingProvider> BlockInspector<'inspector, T> {
    pub fn new(
        parser: &'inspector Parser<'inspector, T>,
        database: &'inspector Libmdbx,
        classifier: &'inspector Classifier<'_, T>,
        inspectors: &'inspector [&'inspector Box<dyn Inspector>],
        block_number: u64,
    ) -> Self {
        Self {
            block_number,
            inspectors,
            parser,
            database,
            classifier,
            composer_future: None,
            classifier_future: None,
        }
    }

    fn start_collection(&mut self) {
        trace!(target:"brontes", block_number = self.block_number, "starting collection of data");
        let parser_fut = self.parser.execute(self.block_number);
        let labeller_fut = self.get_metadata(self.block_number);

        let classifier_fut = Box::pin(async {
            let (traces, header) = parser_fut.await.unwrap().unwrap();
            debug!("Got {} traces + header", traces.len());
            let block_number = header.number;
            let (extra_data, tree) = self.classifier.build_block_tree(traces, header).await;

            MissingDecimals::new(
                self.parser.get_tracer(),
                self.database,
                block_number,
                extra_data.tokens_decimal_fill,
            )
            .await;

            let meta = labeller_fut.unwrap();

            (meta, tree)
        });

        self.classifier_future = Some(classifier_fut);
    }

    fn on_inspectors_finish(&mut self, results: (MevBlock, Vec<(ClassifiedMev, SpecificMev)>)) {
        trace!(
            block_number = self.block_number,
            "inserting the collected results \n {:#?}",
            results
        );

        let data = MevBlocksData {
            block_number: results.0.block_number,
            mev_blocks:   MevBlockWithClassified { block: results.0, mev: results.1 },
        };
        if self
            .database
            .write_table::<MevBlocks, MevBlocksData>(&vec![data])
            .is_err()
        {
            error!("failed to insert classified mev to Libmdbx");
        }
    }

    fn progress_futures(&mut self, cx: &mut Context<'_>) {
        if let Some(mut collection_fut) = self.classifier_future.take() {
            match collection_fut.poll_unpin(cx) {
                Poll::Ready((meta_data, tree)) => {
                    self.composer_future = Some(Box::pin(Composer::new(
                        self.inspectors,
                        tree.into(),
                        meta_data.into(),
                    )));
                }
                Poll::Pending => {
                    self.classifier_future = Some(collection_fut);
                    return
                }
            }
        }

        if let Some(mut inner) = self.composer_future.take() {
            if let Poll::Ready(data) = inner.poll_unpin(cx) {
                info!(
                    target:"brontes",
                    "Finished processing block: {} \n- MEV Count: {}\n- Finalized ETH Price: \
                     ${:.2}\n- Cumulative Gas Used: {}\n- Cumulative Gas Paid: {}\n- Total Bribe: \
                     {}\n- Cumulative MEV Priority Fee Paid: {}\n- Builder Address: {:?}\n- \
                     Builder ETH Profit: {}\n- Builder Finalized Profit (USD): ${:.2}\n- Proposer \
                     Fee Recipient: {:?}\n- Proposer MEV Reward: {:?}\n- Proposer Finalized \
                     Profit (USD): {:?}\n- Cumulative MEV Finalized Profit (USD): ${:.2}\n",
                    data.0.block_number,
                    data.0.mev_count,
                    data.0.finalized_eth_price,
                    data.0.cumulative_gas_used,
                    data.0.cumulative_gas_paid,
                    data.0.total_bribe,
                    data.0.cumulative_mev_priority_fee_paid,
                    data.0.builder_address,
                    data.0.builder_eth_profit,
                    data.0.builder_finalized_profit_usd,
                    data.0
                        .proposer_fee_recipient
                        .map_or(Address::ZERO.to_string(), |v| format!("{:?}", v)),
                    data.0
                        .proposer_mev_reward
                        .map_or("None".to_string(), |v| v.to_string()),
                    data.0
                        .proposer_finalized_profit_usd
                        .map_or("None".to_string(), |v| format!("{:.2}", v)),
                    data.0.cumulative_mev_finalized_profit_usd
                );
                self.on_inspectors_finish(data);
            } else {
                self.composer_future = Some(inner);
            }
        }
    }

    pub fn get_metadata(&self, block_num: u64) -> eyre::Result<MetadataCombined> {
        let tx = self.database.ro_tx()?;
        let block_meta: MetadataInner = tx
            .get::<MetadataTable>(block_num)?
            .ok_or_else(|| reth_db::DatabaseError::Read(-1))?;

        /*
        let db_cex_quotes = CexPriceMap(
            tx.get::<CexPrice>(block_num)?
                .ok_or_else(|| reth_db::DatabaseError::Read(-1))?
                .0,
        );*/

        let db_cex_quotes: CexPriceMap = CexPriceMap::default();

        let eth_prices = if let Some(eth_usdt) = db_cex_quotes.get_quote(&Pair(
            Address::from_str(WETH_ADDRESS).unwrap(),
            Address::from_str(USDT_ADDRESS).unwrap(),
        )) {
            eth_usdt
        } else {
            db_cex_quotes
                .get_quote(&Pair(
                    Address::from_str(WETH_ADDRESS).unwrap(),
                    Address::from_str(USDC_ADDRESS).unwrap(),
                ))
                .unwrap_or_default()
        };

        let mut cex_quotes = CexPriceMap::new();
        db_cex_quotes.0.into_iter().for_each(|(pair, quote)| {
            cex_quotes.0.insert(
                pair,
                quote
                    .into_iter()
                    .map(|q| CexQuote {
                        exchange:  q.exchange,
                        timestamp: q.timestamp,
                        price:     q.price,
                        token0:    q.token0,
                    })
                    .collect::<Vec<_>>(),
            );
        });

        let dex_quotes = Vec::new();
        let key_range = make_filter_key_range(block_num);
        let _db_dex_quotes = tx
            .cursor_read::<DexPrice>()?
            .walk_range(key_range.0..key_range.1)?
            .flat_map(|inner| {
                if let Ok((key, val)) = inner.map(|row| (row.0, row.1)) {
                    //dex_quotes.push(Default::default());
                    Some(key)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        //.get::<DexPrice>(block_num)?
        //.ok_or_else(|| reth_db::DatabaseError::Read(-1))?;

        Ok(MetadataCombined {
            db:         MetadataNoDex {
                block_num,
                block_hash: block_meta.block_hash,
                relay_timestamp: block_meta.relay_timestamp,
                p2p_timestamp: block_meta.p2p_timestamp,
                proposer_fee_recipient: block_meta.proposer_fee_recipient,
                proposer_mev_reward: block_meta.proposer_mev_reward,
                cex_quotes,
                eth_prices: max(eth_prices.price.0, eth_prices.price.1),
                block_timestamp: block_meta.block_timestamp,
                mempool_flow: block_meta.mempool_flow.into_iter().collect(),
            },
            dex_quotes: DexQuotes(dex_quotes),
        })
    }
}

impl<T: TracingProvider> Future for BlockInspector<'_, T> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // If the classifier_future is None (not started yet), start the collection
        // phase
        if self.classifier_future.is_none() && self.composer_future.is_none() {
            self.start_collection();
        }

        self.progress_futures(cx);

        // Decide when to finish the BlockInspector's future.
        // Finish when both classifier and insertion futures are done.
        if self.classifier_future.is_none() && self.composer_future.is_none() {
            info!(
                target:"brontes",
                block_number = self.block_number, "finished inspecting block");

            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}
