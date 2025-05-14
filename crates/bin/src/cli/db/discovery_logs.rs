use std::{collections::HashMap, path::Path, str::FromStr, sync::Arc};

use alloy_primitives::Address;
use alloy_rpc_types::RawLog;
use alloy_sol_macro::sol;
use alloy_sol_types::SolEvent;
use brontes_core::decoding::LogParser as DLogParser;
use brontes_metrics::ParserMetricsListener;
use brontes_types::{init_thread_pools, Protocol, UnboundedYapperReceiver};
use clap::Parser;
use futures::StreamExt;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressState, ProgressStyle};
use itertools::Itertools;
use reth_rpc_types::Filter;
use tokio::sync::mpsc::unbounded_channel;

use crate::{
    cli::{get_env_vars, get_tracing_provider, load_read_only_database, static_object},
    discovery_logs_only::DiscoveryLogsExecutor,
    runner::CliContext,
};

sol!(
    #![sol(all_derives)]
    BalancerV2,
    "../brontes-classifier/classifier-abis/balancer/BalancerV2Vault.json"
);

sol!(
    #![sol(all_derives)]
    UniswapV2,
    "../brontes-classifier/classifier-abis/UniswapV2Factory.json"
);
sol!(
    #![sol(all_derives)]
    UniswapV3,
    "../brontes-classifier/classifier-abis/UniswapV3Factory.json"
);

sol!(
    #![sol(all_derives)]
    UniswapV4,
    "../brontes-classifier/classifier-abis/UniswapV4.json"
);
sol!(
    #![sol(all_derives)]
    CamelotV3,
    "../brontes-classifier/classifier-abis/Algebra1_9Factory.json"
);
sol!(
    #![sol(all_derives)]
    FluidDEX,
    "../brontes-classifier/classifier-abis/fluid/FluidDexFactory.json"
);

#[derive(Debug, Parser)]
pub struct DiscoveryLogsFill {
    /// Start Block
    #[arg(long, short)]
    pub start_block: Option<u64>,
    /// Max number of tasks to run concurrently
    #[arg(long, short)]
    pub max_tasks:   Option<usize>,
}

impl DiscoveryLogsFill {
    pub async fn execute(self, brontes_db_path: String, ctx: CliContext) -> eyre::Result<()> {
        let db_path = get_env_vars()?;

        let max_tasks = self.max_tasks.unwrap_or(num_cpus::get_physical());
        init_thread_pools(max_tasks);

        let libmdbx =
            static_object(load_read_only_database(&ctx.task_executor, brontes_db_path).await?);

        let tracer = Arc::new(get_tracing_provider(
            Path::new(&db_path),
            max_tasks as u64,
            ctx.task_executor.clone(),
        ));

        let balancer_v2_filter = Filter::new()
            .address(Address::from_str("0xba12222222228d8ba445958a75a0704d566bf2c8").unwrap())
            .event_signature(BalancerV2::TokensRegistered::SIGNATURE_HASH);
        let uniswap_v2_filter = Filter::new()
            .address(Address::from_str("0xf1D7CC64Fb4452F05c498126312eBE29f30Fbcf9").unwrap())
            .event_signature(UniswapV2::PairCreated::SIGNATURE_HASH);
        let camelot_v2_filter = Filter::new()
            .address(Address::from_str("0x6EcCab422D763aC031210895C81787E87B43A652").unwrap())
            .event_signature(UniswapV2::PairCreated::SIGNATURE_HASH);
        let camelot_v3_filter = Filter::new()
            .address(Address::from_str("0x1a3c9B1d2F0529D97f2afC5136Cc23e58f1FD35B").unwrap())
            .event_signature(CamelotV3::Pool::SIGNATURE_HASH);
        let uniswap_v3_filter = Filter::new()
            .address(Address::from_str("0x7858E59e0C01EA06D73002144C0a530770217229").unwrap())
            .event_signature(UniswapV3::PoolCreated::SIGNATURE_HASH);
        let uniswap_v4_filter = Filter::new()
            .address(Address::from_str("0x360E68faCcca8cA495c1B759Fd9EEe466db9FB32").unwrap())
            .event_signature(UniswapV4::Initialize::SIGNATURE_HASH);
        let fluid_dex_filter = Filter::new()
            .address(Address::from_str("0x46978CD477A496028A18c02F07ab7F35EDBa5A54").unwrap())
            .event_signature(FluidDEX::DexT1Deployed::SIGNATURE_HASH);

        let mut filters: HashMap<Protocol, Filter> = HashMap::new();
        filters.insert(Protocol::BalancerV2, balancer_v2_filter);
        filters.insert(Protocol::UniswapV2, uniswap_v2_filter);
        filters.insert(Protocol::CamelotV3, camelot_v3_filter);
        filters.insert(Protocol::UniswapV3, uniswap_v3_filter);
        filters.insert(Protocol::UniswapV4, uniswap_v4_filter);
        filters.insert(Protocol::FluidDEX, fluid_dex_filter);

        let parser = static_object(DLogParser::new(libmdbx, tracer.clone(), filters).await);

        let start_block = if let Some(s) = self.start_block {
            s
        } else {
            libmdbx.client.max_traced_block().await?
        };
        let end_block = parser.get_latest_block_number().await.unwrap();

        let bar = ProgressBar::with_draw_target(
            Some(end_block - start_block),
            ProgressDrawTarget::stderr_with_hz(100),
        );
        let style = ProgressStyle::default_bar()
            .template(
                "{msg}\n[{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} blocks \
                 ({percent}%) | ETA: {eta}",
            )
            .expect("Invalid progress bar template")
            .progress_chars("█>-")
            .with_key("eta", |state: &ProgressState, f: &mut dyn std::fmt::Write| {
                write!(f, "{:.1}s", state.eta().as_secs_f64()).unwrap()
            })
            .with_key("percent", |state: &ProgressState, f: &mut dyn std::fmt::Write| {
                write!(f, "{:.1}", state.fraction() * 100.0).unwrap()
            });
        bar.set_style(style);
        bar.set_message("Processing blocks:");

        let chunks = (start_block..=end_block)
            .chunks(max_tasks)
            .into_iter()
            .map(|mut c| {
                let start = c.next().unwrap();
                let end_block = c.last().unwrap_or(start_block);
                (start, end_block)
            })
            .collect_vec();

        futures::stream::iter(chunks)
            .map(|(start_block, end_block)| {
                let bar = bar.clone();
                ctx.task_executor
                    .spawn_critical_with_graceful_shutdown_signal(
                        "DiscoveryLogs",
                        |shutdown| async move {
                            DiscoveryLogsExecutor::new(start_block, end_block, libmdbx, parser, bar)
                                .run_until_graceful_shutdown(shutdown)
                                .await
                        },
                    )
            })
            .buffer_unordered(max_tasks)
            .collect::<Vec<_>>()
            .await;
        Ok(())
    }
}
