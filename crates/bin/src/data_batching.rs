use std::{
    cmp::max,
    collections::HashMap,
    pin::Pin,
    str::FromStr,
    sync::Arc,
    task::{Context, Poll},
};

use alloy_primitives::Address;
use brontes_classifier::Classifier;
use brontes_core::{
    decoding::{Parser, TracingProvider},
    missing_decimals::MissingDecimals,
};
use brontes_database::libmdbx::{
    tables::{CexPrice, DexPrice, Metadata, MevBlocks},
    types::{dex_price::DexPriceData, mev_block::MevBlocksData, LibmdbxData},
    Libmdbx,
};
use brontes_inspect::{composer::compose_mev_results, Inspector};
use brontes_pricing::{BrontesBatchPricer, GraphManager};
use brontes_types::{
    classified_mev::{ClassifiedMev, MevBlock, SpecificMev},
    constants::{USDC_ADDRESS, USDT_ADDRESS, WETH_ADDRESS},
    db::{
        cex::{CexPriceMap, CexQuote},
        dex::{DexQuote, DexQuotes},
        metadata::{MetadataCombined, MetadataInner, MetadataNoDex},
        mev_block::MevBlockWithClassified,
    },
    extra_processing::Pair,
    normalized_actions::Actions,
    structured_trace::TxTrace,
    tree::BlockTree,
};
use futures::{stream::FuturesUnordered, Future, FutureExt, Stream, StreamExt};
use reth_db::DatabaseError;
use reth_primitives::Header;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

type CollectionFut<'a> =
    Pin<Box<dyn Future<Output = (BlockTree<Actions>, MetadataNoDex)> + Send + 'a>>;

pub struct DataBatching<'db, T: TracingProvider + Clone> {
    parser:     &'db Parser<'db, T>,
    classifier: Classifier<'db, T>,

    collection_future: Option<CollectionFut<'db>>,
    pricer:            WaitingForPricerFuture<T>,

    processing_futures: FuturesUnordered<Pin<Box<dyn Future<Output = ()> + Send + 'db>>>,

    current_block: u64,
    end_block:     u64,
    batch_id:      u64,

    libmdbx:    &'static Libmdbx,
    inspectors: &'db [&'db Box<dyn Inspector>],
}

impl<'db, T: TracingProvider + Clone> DataBatching<'db, T> {
    pub fn new(
        quote_asset: alloy_primitives::Address,
        max_pool_loading_tasks: usize,
        batch_id: u64,
        start_block: u64,
        end_block: u64,
        parser: &'db Parser<'db, T>,
        libmdbx: &'static Libmdbx,
        inspectors: &'db [&'db Box<dyn Inspector>],
    ) -> Self {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let classifier = Classifier::new(libmdbx, tx, parser.get_tracer());

        let pairs = libmdbx.protocols_created_before(start_block).unwrap();

        let rest_pairs = libmdbx
            .protocols_created_range(start_block + 1, end_block)
            .unwrap()
            .into_iter()
            .flat_map(|(_, pools)| {
                pools
                    .into_iter()
                    .map(|(addr, protocol, pair)| (addr, (protocol, pair)))
                    .collect::<Vec<_>>()
            })
            .collect::<HashMap<_, _>>();

        let pair_graph = GraphManager::init_from_db_state(
            pairs,
            HashMap::default(),
            Box::new(|block, pair| libmdbx.try_load_pair_before(block, pair).ok()),
            Box::new(|block, pair, edges| {
                if libmdbx.save_pair_at(block, pair, edges).is_err() {
                    error!("failed to save subgraph to db");
                }
            }),
        );

        let pricer = BrontesBatchPricer::new(
            max_pool_loading_tasks,
            quote_asset,
            pair_graph,
            rx,
            parser.get_tracer(),
            start_block,
            rest_pairs,
        );

        let pricer = WaitingForPricerFuture::new(pricer);

        Self {
            collection_future: None,
            processing_futures: FuturesUnordered::default(),
            parser,
            classifier,
            pricer,
            current_block: start_block,
            end_block,
            batch_id,
            libmdbx,
            inspectors,
        }
    }

    fn on_parser_resolve(
        meta: MetadataNoDex,
        traces: Vec<TxTrace>,
        header: Header,
        classifier: Classifier<'db, T>,
        tracer: Arc<T>,
        libmdbx: &'db Libmdbx,
    ) -> CollectionFut<'db> {
        Box::pin(async move {
            let number = header.number;
            let (extra, tree) = classifier.build_block_tree(traces, header).await;
            MissingDecimals::new(tracer, libmdbx, number, extra.tokens_decimal_fill).await;

            (tree, meta)
        })
    }

    fn start_next_block(&mut self) {
        let parser = self.parser.execute(self.current_block);
        let meta = self.get_metadata_no_dex(self.current_block).unwrap();

        let classifier = self.classifier.clone();

        let fut = Box::pin(parser.then(|x| {
            let (traces, header) = x.unwrap().unwrap();
            Self::on_parser_resolve(
                meta,
                traces,
                header,
                classifier,
                self.parser.get_tracer(),
                self.libmdbx,
            )
        }));

        self.collection_future = Some(fut);
    }

    fn on_price_finish(&mut self, tree: BlockTree<Actions>, meta: MetadataCombined) {
        info!(target:"brontes","dex pricing finished");
        self.processing_futures.push(Box::pin(process_results(
            self.libmdbx,
            self.inspectors,
            tree.into(),
            meta.into(),
        )));
    }

    pub fn get_metadata_no_dex(&self, block_num: u64) -> eyre::Result<MetadataNoDex> {
        let tx = self.libmdbx.ro_tx()?;

        let block_meta: MetadataInner = tx
            .get::<Metadata>(block_num)?
            .ok_or_else(|| reth_db::DatabaseError::Read(-1))?;

        /*
                let db_cex_quotes: CexPriceMap = tx
                    .get::<CexPrice>(block_num)?
                    .ok_or_else(|| reth_db::DatabaseError::Read(-1))?;
        */

        let db_cex_quotes: CexPriceMap = match tx
            .get::<CexPrice>(block_num)?
            .ok_or_else(|| reth_db::DatabaseError::Read(-1))
        {
            Ok(map) => map,
            Err(e) => {
                warn!(target:"brontes","failed to read CexPrice db table for block {} -- {:?}", block_num, e);
                CexPriceMap::default()
            }
        };

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

        Ok(MetadataNoDex {
            block_num,
            block_hash: block_meta.block_hash,
            relay_timestamp: block_meta.relay_timestamp,
            p2p_timestamp: block_meta.p2p_timestamp,
            proposer_fee_recipient: block_meta.proposer_fee_recipient,
            proposer_mev_reward: block_meta.proposer_mev_reward,
            cex_quotes,
            eth_prices: max(eth_prices.price.0, eth_prices.price.1),

            mempool_flow: block_meta.mempool_flow.into_iter().collect(),
            block_timestamp: block_meta.block_timestamp,
        })
    }
}

impl<T: TracingProvider + Clone> Future for DataBatching<'_, T> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // poll pricer
        if let Poll::Ready(Some((tree, meta))) = self.pricer.poll_next_unpin(cx) {
            if meta.block_num == self.end_block {
                info!(
                    batch_id = self.batch_id,
                    end_block = self.end_block,
                    "batch finished completed"
                );
            }

            self.on_price_finish(tree, meta);
        }

        // progress collection future,
        if let Some(mut future) = self.collection_future.take() {
            if let Poll::Ready((tree, meta)) = future.poll_unpin(cx) {
                debug!("built tree");
                let block = self.current_block;
                self.pricer.add_pending_inspection(block, tree, meta);
            } else {
                self.collection_future = Some(future);
            }
        } else if self.current_block != self.end_block {
            self.current_block += 1;
            self.start_next_block();
        }

        // If we have reached end block and there is only 1 pending tree left,
        // send the close message to indicate to the dex pricer that it should
        // return. This will spam it till the pricer closes but this is needed as it
        // could take multiple polls until the pricing is done for the final
        // block.
        if self.pricer.pending_trees.len() <= 1 && self.current_block == self.end_block {
            self.classifier.close();
        }
        // poll insertion
        while let Poll::Ready(Some(_)) = self.processing_futures.poll_next_unpin(cx) {}

        // return condition
        if self.current_block == self.end_block
            && self.collection_future.is_none()
            && self.processing_futures.is_empty()
            && self.pricer.is_done()
        {
            return Poll::Ready(())
        }

        cx.waker().wake_by_ref();
        Poll::Pending
    }
}

pub struct WaitingForPricerFuture<T: TracingProvider> {
    handle:        JoinHandle<(BrontesBatchPricer<T>, Option<(u64, DexQuotes)>)>,
    pending_trees: HashMap<u64, (BlockTree<Actions>, MetadataNoDex)>,
}

impl<T: TracingProvider> WaitingForPricerFuture<T> {
    pub fn new(mut pricer: BrontesBatchPricer<T>) -> Self {
        let future = Box::pin(async move {
            let res = pricer.next().await;
            (pricer, res)
        });

        let rt_handle = tokio::runtime::Handle::current();
        let move_handle = rt_handle.clone();

        let handle = rt_handle.spawn_blocking(move || move_handle.block_on(future));

        Self { handle, pending_trees: HashMap::default() }
    }

    pub fn is_done(&self) -> bool {
        self.pending_trees.is_empty()
    }

    fn resechedule(&mut self, mut pricer: BrontesBatchPricer<T>) {
        let future = Box::pin(async move {
            let res = pricer.next().await;
            (pricer, res)
        });

        let rt_handle = tokio::runtime::Handle::current();
        let move_handle = rt_handle.clone();

        self.handle = rt_handle.spawn_blocking(move || move_handle.block_on(future));
    }

    pub fn add_pending_inspection(
        &mut self,
        block: u64,
        tree: BlockTree<Actions>,
        meta: MetadataNoDex,
    ) {
        assert!(
            self.pending_trees.insert(block, (tree, meta)).is_none(),
            "traced a duplicate block"
        );
    }
}

impl<T: TracingProvider> Stream for WaitingForPricerFuture<T> {
    type Item = (BlockTree<Actions>, MetadataCombined);

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if let Poll::Ready(handle) = self.handle.poll_unpin(cx) {
            let (pricer, inner) = handle.unwrap();
            self.resechedule(pricer);

            if let Some((block, prices)) = inner {
                info!(target:"brontes","Collected dex prices for block: {}", block);

                let Some((tree, meta)) = self.pending_trees.remove(&block) else {
                    return Poll::Ready(None)
                };

                let finalized_meta = meta.into_finalized_metadata(prices);
                return Poll::Ready(Some((tree, finalized_meta)))
            } else {
                // means we have completed chunks
                return Poll::Ready(None)
            }
        }

        Poll::Pending
    }
}

async fn process_results(
    db: &Libmdbx,
    inspectors: &[&Box<dyn Inspector>],
    tree: Arc<BlockTree<Actions>>,
    metadata: Arc<MetadataCombined>,
) {
    let (block_details, mev_details) =
        compose_mev_results(inspectors, tree, metadata.clone()).await;

    if let Err(e) = insert_quotes(db, metadata.block_num.clone(), metadata.dex_quotes.clone()) {
        tracing::error!(err=%e, block_num=metadata.block_num, "failed to insert dex pricing and state into db");
    }

    insert_mev_results(db, block_details, mev_details);
}

fn insert_mev_results(
    database: &Libmdbx,
    block_details: MevBlock,
    mev_details: Vec<(ClassifiedMev, SpecificMev)>,
) {
    info!(
        target:"brontes",
        "Finished processing block: {} \n- MEV Count: {}\n- Finalized ETH Price: \
         ${:.2}\n- Cumulative Gas Used: {}\n- Cumulative Gas Paid: {}\n- Total Bribe: \
         {}\n- Cumulative MEV Priority Fee Paid: {}\n- Builder Address: {:?}\n- Builder \
         ETH Profit: {}\n- Builder Finalized Profit (USD): ${:.2}\n- Proposer Fee \
         Recipient: {:?}\n- Proposer MEV Reward: {:?}\n- Proposer Finalized Profit (USD): \
        {:?}\n- Cumulative MEV Finalized Profit (USD): ${:.2}\n- Possibly Missed Mev: {}",
        block_details.block_number,
        block_details.mev_count,
        block_details.finalized_eth_price,
        block_details.cumulative_gas_used,
        block_details.cumulative_gas_paid,
        block_details.total_bribe,
        block_details.cumulative_mev_priority_fee_paid,
        block_details.builder_address,
        block_details.builder_eth_profit,
        block_details.builder_finalized_profit_usd,
        block_details
            .proposer_fee_recipient
            .unwrap_or(Address::ZERO),
        block_details
            .proposer_mev_reward
            .map_or("None".to_string(), |v| v.to_string()),
        block_details
            .proposer_finalized_profit_usd
            .map_or("None".to_string(), |v| format!("{:.2}", v)),
        block_details.cumulative_mev_finalized_profit_usd,
    block_details
        .possible_missed_arbs
        .iter()
        .map(|arb| format!("https://etherscan.io/tx/{arb:?}"))
        .fold(String::new(), |acc, arb| acc + &arb + "\n")
    );

    let data = MevBlocksData {
        block_number: block_details.block_number,
        mev_blocks:   MevBlockWithClassified { block: block_details, mev: mev_details },
    };

    if database
        .write_table::<MevBlocks, MevBlocksData>(&vec![data])
        .is_err()
    {
        error!("failed to insert classified data into libmdx");
    }
}

fn insert_quotes(database: &Libmdbx, block_num: u64, quotes: DexQuotes) -> eyre::Result<()> {
    let mut data = quotes
        .0
        .into_iter()
        .enumerate()
        .filter(|(_, v)| v.is_some())
        .map(|(idx, value)| DexPriceData {
            block_number: block_num,
            tx_idx:       idx as u16,
            quote:        DexQuote(value.unwrap()),
        })
        .collect::<Vec<_>>();

    data.sort_by(|a, b| a.tx_idx.cmp(&b.tx_idx));
    data.sort_by(|a, b| a.block_number.cmp(&b.block_number));

    database.update_db(|tx| {
        let mut cursor = tx.cursor_write::<DexPrice>()?;

        data.into_iter()
            .map(|entry| {
                let (key, val) = entry.into_key_val();
                cursor.upsert(key, val)?;
                Ok(())
            })
            .collect::<Result<Vec<_>, DatabaseError>>()
    })??;

    Ok(())
}
