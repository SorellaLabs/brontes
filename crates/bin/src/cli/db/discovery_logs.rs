use std::{collections::HashMap, path::Path, str::FromStr, sync::Arc};

use alloy_primitives::Address;
use alloy_sol_macro::sol;
use alloy_sol_types::SolEvent;
use brontes_core::decoding::LogParser as DLogParser;
use brontes_types::{
    constants::arbitrum::{
        BALANCER_V2_VAULT_ADDRESS, CAMELOT_V2_FACTORY_ADDRESS, CAMELOT_V3_FACTORY_ADDRESS,
        FLUID_DEX_FACTORY_ADDRESS, PANCAKESWAP_V2_FACTORY_ADDRESS, PANCAKESWAP_V3_FACTORY_ADDRESS,
        SUSHISWAP_V2_FACTORY_ADDRESS, SUSHISWAP_V3_FACTORY_ADDRESS, UNISWAP_V2_FACTORY_ADDRESS,
        UNISWAP_V3_FACTORY_ADDRESS, UNISWAP_V4_FACTORY_ADDRESS,
    },
    init_thread_pools, Protocol,
};
use clap::Parser;
use futures::StreamExt;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressState, ProgressStyle};
use itertools::Itertools;
use reth_rpc_types::Filter;

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
    /// Block range per request (defaults to alchemy block range limit = 10,000)
    #[arg(long, short, default_value_t = 10_000)]
    pub range_size: usize,
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
            .address(BALANCER_V2_VAULT_ADDRESS)
            .event_signature(BalancerV2::TokensRegistered::SIGNATURE_HASH);
        let uniswap_v2_filter = Filter::new()
            .address(UNISWAP_V2_FACTORY_ADDRESS)
            .event_signature(UniswapV2::PairCreated::SIGNATURE_HASH);
        let sushiswap_v2_filter = Filter::new()
            .address(SUSHISWAP_V2_FACTORY_ADDRESS)
            .event_signature(UniswapV2::PairCreated::SIGNATURE_HASH);
        let pancakeswap_v2_filter = Filter::new()
            .address(PANCAKESWAP_V2_FACTORY_ADDRESS)
            .event_signature(UniswapV2::PairCreated::SIGNATURE_HASH);
        let sushiswap_v3_filter = Filter::new()
            .address(SUSHISWAP_V3_FACTORY_ADDRESS)
            .event_signature(UniswapV3::PoolCreated::SIGNATURE_HASH);
        let pancakeswap_v3_filter = Filter::new()
            .address(PANCAKESWAP_V3_FACTORY_ADDRESS)
            .event_signature(UniswapV3::PoolCreated::SIGNATURE_HASH);
        let camelot_v2_filter = Filter::new()
            .address(CAMELOT_V2_FACTORY_ADDRESS)
            .event_signature(UniswapV2::PairCreated::SIGNATURE_HASH);
        let camelot_v3_filter = Filter::new()
            .address(CAMELOT_V3_FACTORY_ADDRESS)
            .event_signature(CamelotV3::Pool::SIGNATURE_HASH);
        let uniswap_v3_filter = Filter::new()
            .address(UNISWAP_V3_FACTORY_ADDRESS)
            .event_signature(UniswapV3::PoolCreated::SIGNATURE_HASH);
        let uniswap_v4_filter = Filter::new()
            .address(UNISWAP_V4_FACTORY_ADDRESS)
            .event_signature(UniswapV4::Initialize::SIGNATURE_HASH);
        let fluid_dex_filter = Filter::new()
            .address(FLUID_DEX_FACTORY_ADDRESS)
            .event_signature(FluidDEX::DexT1Deployed::SIGNATURE_HASH);

        let mut filters: HashMap<Protocol, Filter> = HashMap::new();
        filters.insert(Protocol::BalancerV2, balancer_v2_filter);
        filters.insert(Protocol::UniswapV2, uniswap_v2_filter);
        filters.insert(Protocol::SushiSwapV2, sushiswap_v2_filter);
        filters.insert(Protocol::PancakeSwapV2, pancakeswap_v2_filter);
        filters.insert(Protocol::SushiSwapV3, sushiswap_v3_filter);
        filters.insert(Protocol::PancakeSwapV3, pancakeswap_v3_filter);
        filters.insert(Protocol::UniswapV3, uniswap_v3_filter);
        filters.insert(Protocol::UniswapV4, uniswap_v4_filter);
        filters.insert(Protocol::CamelotV2, camelot_v2_filter);
        filters.insert(Protocol::CamelotV3, camelot_v3_filter);
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
            .chunks(self.range_size)
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
