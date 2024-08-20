use std::{path::Path, sync::Arc};

use brontes_classifier::Classifier;
use brontes_core::{decoding::Parser as DParser, LibmdbxReader};
use brontes_database::{clickhouse::cex_config::CexDownloadConfig, libmdbx::LibmdbxReadWriter};
use brontes_inspect::{cex_dex::quotes::CexDexQuotesInspector, shared_utils::SharedInspectorUtils};
use brontes_types::{
    constants::USDT_ADDRESS,
    db::cex::quotes::FeeAdjustedQuote,
    init_thread_pools,
    mev::MevType,
    normalized_actions::{Action, NormalizedSwap},
    traits::TracingProvider,
    BlockTree, ToFloatNearest, TreeCollector, TreeSearchBuilder,
};
use clap::{Parser, Subcommand};
use colored::*;
use malachite::Rational;
use prettytable::{Cell, Row, Table};
use reth_tracing_ext::TracingClient;
use tokio::sync::mpsc::unbounded_channel;

use crate::{
    cli::{get_env_vars, get_tracing_provider, load_libmdbx, static_object},
    runner::CliContext,
};

#[derive(Debug, Parser)]
pub struct CexQuery {
    #[clap(subcommand)]
    pub command: CexQueryCommands,
}

impl CexQuery {
    pub async fn execute(self, brontes_db_endpoint: String, ctx: CliContext) -> eyre::Result<()> {
        println!("Executing CexQuery");
        match self.command {
            CexQueryCommands::Quotes(cex_db) => cex_db.execute(brontes_db_endpoint, ctx).await,
            CexQueryCommands::Trades(cex_db) => cex_db.execute(brontes_db_endpoint, ctx).await,
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum CexQueryCommands {
    /// Query Cex Quotes from the Sorella DB
    #[command(name = "quotes")]
    Quotes(CexQuotesDebug),
    /// Query Cex Trades from the Sorella DB
    #[command(name = "trades")]
    Trades(CexTradesDebug),
}

#[derive(Debug, Parser)]
pub struct CexQuotesDebug {
    /// The tx hash to debug
    #[clap(long, short)]
    pub tx_hash: String,
}

impl CexQuotesDebug {
    pub async fn execute(self, brontes_db_endpoint: String, ctx: CliContext) -> eyre::Result<()> {
        init_thread_pools(10);
        println!("Executing Quotes command");

        let task_executor = ctx.task_executor;
        let reth_db_path = get_env_vars().expect("Failed to get reth db path");

        let libmdbx = static_object(
            load_libmdbx(&task_executor, brontes_db_endpoint).expect("Failed to load libmdbx"),
        );

        let (metrics_tx, _metrics_rx) = unbounded_channel();

        let tracer = get_tracing_provider(Path::new(&reth_db_path), 10, task_executor.clone());
        let parser = static_object(DParser::new(metrics_tx, libmdbx, tracer.clone()).await);

        let tx_tree = self.get_block_tree(libmdbx, parser).await;

        println!("Got block tree");
        let tx_tree = Arc::new(tx_tree);

        let cex_config = CexDownloadConfig::default();

        print!("Getting metadata...");
        let metadata = libmdbx
            .get_metadata(tx_tree.header.number, USDT_ADDRESS)
            .expect("Failed to get metadata");

        println!("Got metadata");

        let inspector = CexDexQuotesInspector::new(
            USDT_ADDRESS,
            libmdbx,
            &cex_config.exchanges_to_use,
            0,
            None,
        );

        tx_tree
            .clone()
            .collect_all(TreeSearchBuilder::default().with_actions([
                Action::is_swap,
                Action::is_transfer,
                Action::is_eth_transfer,
                Action::is_aggregator,
            ]))
            .for_each(|(tx, swaps)| {
                print!("Processing tx {}...", tx);

                let tx_info = tx_tree
                    .get_tx_info(tx, &libmdbx)
                    .expect("Failed to get tx info");

                let (mut dex_swaps, rem): (Vec<_>, _) = inspector
                    .utils
                    .flatten_nested_actions(swaps.into_iter(), &|action| action.is_swap())
                    .split_return_rem(Action::try_swaps_merged);

                let transfers: Vec<_> = rem.into_iter().split_actions(Action::try_transfer);

                if dex_swaps.is_empty() {
                    if let Some(extra) = inspector.utils.cex_try_convert_transfer_to_swap(
                        transfers,
                        &tx_info,
                        MevType::CexDexQuotes,
                    ) {
                        dex_swaps.push(extra);
                    }
                }

                let merged_swaps =
                    SharedInspectorUtils::<'_, LibmdbxReadWriter>::cex_merge_possible_swaps(
                        dex_swaps.clone(),
                    );
                let quotes = inspector.cex_quotes_for_swap(&merged_swaps, &metadata, 0, None);

                print_report(&dex_swaps, &merged_swaps, &quotes);
            });

        println!("Done");

        Ok(())
    }

    async fn get_block_tree(
        &self,
        libmdbx: &LibmdbxReadWriter,
        parser: &DParser<TracingClient, LibmdbxReadWriter>,
    ) -> BlockTree<Action> {
        println!("Creating inspector utils");

        let (unbounded_tx, _unbounded_rx) = unbounded_channel();

        let (block, tx_idx) = parser
            .get_tracer()
            .block_and_tx_index(self.tx_hash.parse().expect("Invalid Tx hash"))
            .await
            .expect("Failed to get block and tx index");

        let (traces, header) = parser
            .execute(block, tx_idx, None)
            .await
            .expect("Failed to execute block");

        let classifier = Classifier::new(libmdbx, unbounded_tx, parser.get_tracer());

        let trace = traces.first().expect("No traces found");

        classifier
            .build_block_tree(vec![trace.clone()], header, false)
            .await
    }
}

pub fn print_report(
    original_swaps: &[NormalizedSwap],
    merged_swaps: &[NormalizedSwap],
    cex_quotes: &[Option<FeeAdjustedQuote>],
) {
    println!("{}", "CEX-DEX Comparison Report".bold());
    println!("{}", "=========================\n".bold());

    println!("{}", "1. Original Swaps".underline());
    print_swaps(original_swaps);

    println!("\n{}", "2. Merged Swaps and CEX Quotes Comparison".underline());
    print_detailed_comparison(merged_swaps, cex_quotes);
}

fn print_swaps(swaps: &[NormalizedSwap]) {
    for swap in swaps {
        println!("{}", swap);
    }
}

fn print_detailed_comparison(
    merged_swaps: &[NormalizedSwap],
    cex_quotes: &[Option<FeeAdjustedQuote>],
) {
    let mut table = Table::new();
    table.add_row(Row::new(vec![
        Cell::new("Merged Swap").style_spec("b"),
        Cell::new("CEX Quote").style_spec("b"),
        Cell::new("DEX Rate").style_spec("b"),
        Cell::new("CEX Rate").style_spec("b"),
        Cell::new("Token In Delta").style_spec("b"),
        Cell::new("CEX Prices (Maker/Taker)").style_spec("b"),
    ]));

    for (swap, quote_option) in merged_swaps.iter().zip(cex_quotes.iter()) {
        let (dex_rate, cex_rate, token_in_delta, cex_prices) =
            calculate_comparison(swap, quote_option.as_ref());

        table.add_row(Row::new(vec![
            Cell::new(&swap.to_string()),
            Cell::new(
                &quote_option
                    .as_ref()
                    .map_or("No quote".red().to_string(), |q| {
                        format!("{:?}", q.exchange).green().to_string()
                    }),
            ),
            Cell::new(&format!("{:.8}", dex_rate.to_float())),
            Cell::new(&format_optional_value(cex_rate.map(|r| r.to_float()))),
            Cell::new(&format_optional_value(token_in_delta.map(|d| d.to_float()))),
            Cell::new(&format_cex_prices(cex_prices)),
        ]));
    }

    table.printstd();
}

#[allow(clippy::type_complexity)]
fn calculate_comparison(
    swap: &NormalizedSwap,
    quote: Option<&FeeAdjustedQuote>,
) -> (Rational, Option<Rational>, Option<Rational>, Option<(Rational, Rational)>) {
    let dex_rate = &swap.amount_in / &swap.amount_out;

    quote.map_or((dex_rate.clone(), None, None, None), |q: &FeeAdjustedQuote| {
        let (maker_mid, taker_mid) = q.maker_taker_mid();
        let cex_rate = taker_mid.clone();
        let cex_equivalent_in = &swap.amount_out * &cex_rate;
        let token_in_delta = &cex_equivalent_in - &swap.amount_in;

        (dex_rate, Some(cex_rate), Some(token_in_delta), Some((maker_mid, taker_mid)))
    })
}

fn format_optional_value(value: Option<f64>) -> String {
    value.map_or("N/A".to_string(), |v| format!("{:.8}", v))
}

fn format_cex_prices(prices: Option<(Rational, Rational)>) -> String {
    prices.map_or("N/A".to_string(), |(maker, taker)| {
        format!("M: {:.8} / T: {:.8}", maker.to_float(), taker.to_float())
    })
}

#[derive(Debug, Parser)]
pub struct CexTradesDebug {
    /// The tx hash to debug
    #[clap(long, short)]
    pub tx_hash: String,
}

impl CexTradesDebug {
    pub async fn execute(self, _brontes_db_endpoint: String, _ctx: CliContext) -> eyre::Result<()> {
        Ok(())
    }
}
