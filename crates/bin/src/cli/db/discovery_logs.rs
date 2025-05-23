use std::{collections::HashMap, num::NonZeroU32, path::Path, sync::Arc};

use alloy_primitives::{Address, FixedBytes};
use alloy_sol_macro::sol;
use alloy_sol_types::SolEvent;
use brontes_core::decoding::LogParser as DLogParser;
use brontes_types::{
    constants::arbitrum::{
        BALANCER_V2_VAULT_ADDRESS, CAMELOT_V2_FACTORY_ADDRESS, CAMELOT_V3_FACTORY_ADDRESS,
        FLUID_DEX_FACTORY_ADDRESS, LFJ_V2_1_DEX_FACTORY_ADDRESS, LFJ_V2_2_DEX_FACTORY_ADDRESS,
        PANCAKESWAP_V2_FACTORY_ADDRESS, PANCAKESWAP_V3_FACTORY_ADDRESS,
        SUSHISWAP_V2_FACTORY_ADDRESS, SUSHISWAP_V3_FACTORY_ADDRESS, UNISWAP_V2_FACTORY_ADDRESS,
        UNISWAP_V3_FACTORY_ADDRESS, UNISWAP_V4_FACTORY_ADDRESS,
    },
    init_thread_pools, Protocol,
};
use clap::Parser;
use futures::StreamExt;
use governor::{Quota, RateLimiter};
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressState, ProgressStyle};
use itertools::Itertools;

use crate::{
    cli::{get_env_vars, get_tracing_provider_rpc, load_database, static_object},
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
    LFJV2,
    "../brontes-classifier/classifier-abis/LFJ/ILBFactory.json"
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
    /// End Block
    #[arg(long, short)]
    pub end_block:   Option<u64>,
    /// Max number of tasks to run concurrently
    #[arg(long, short)]
    pub max_tasks:   Option<usize>,
    /// Block range per request (defaults to alchemy block range limit = 10,000)
    #[arg(long, short, default_value_t = 10_000)]
    pub range_size:  usize,

    #[arg(long, short)]
    pub rate_limit: Option<u32>,
}

impl DiscoveryLogsFill {
    fn get_protocol_to_address_map() -> HashMap<Protocol, (Address, FixedBytes<32>)> {
        let mut protocol_to_address = HashMap::new();
        protocol_to_address.insert(
            Protocol::BalancerV2,
            (BALANCER_V2_VAULT_ADDRESS, BalancerV2::TokensRegistered::SIGNATURE_HASH),
        );
        protocol_to_address.insert(
            Protocol::UniswapV2,
            (UNISWAP_V2_FACTORY_ADDRESS, UniswapV2::PairCreated::SIGNATURE_HASH),
        );
        protocol_to_address.insert(
            Protocol::SushiSwapV2,
            (SUSHISWAP_V2_FACTORY_ADDRESS, UniswapV2::PairCreated::SIGNATURE_HASH),
        );
        protocol_to_address.insert(
            Protocol::PancakeSwapV2,
            (PANCAKESWAP_V2_FACTORY_ADDRESS, UniswapV2::PairCreated::SIGNATURE_HASH),
        );
        protocol_to_address.insert(
            Protocol::SushiSwapV3,
            (SUSHISWAP_V3_FACTORY_ADDRESS, UniswapV3::PoolCreated::SIGNATURE_HASH),
        );
        protocol_to_address.insert(
            Protocol::PancakeSwapV3,
            (PANCAKESWAP_V3_FACTORY_ADDRESS, UniswapV3::PoolCreated::SIGNATURE_HASH),
        );
        protocol_to_address.insert(
            Protocol::UniswapV3,
            (UNISWAP_V3_FACTORY_ADDRESS, UniswapV3::PoolCreated::SIGNATURE_HASH),
        );
        protocol_to_address.insert(
            Protocol::UniswapV4,
            (UNISWAP_V4_FACTORY_ADDRESS, UniswapV4::Initialize::SIGNATURE_HASH),
        );
        protocol_to_address.insert(
            Protocol::CamelotV2,
            (CAMELOT_V2_FACTORY_ADDRESS, UniswapV2::PairCreated::SIGNATURE_HASH),
        );
        protocol_to_address.insert(
            Protocol::CamelotV3,
            (CAMELOT_V3_FACTORY_ADDRESS, CamelotV3::Pool::SIGNATURE_HASH),
        );
        protocol_to_address.insert(
            Protocol::FluidDEX,
            (FLUID_DEX_FACTORY_ADDRESS, FluidDEX::DexT1Deployed::SIGNATURE_HASH),
        );
        protocol_to_address.insert(
            Protocol::LFJV2_1,
            (LFJ_V2_1_DEX_FACTORY_ADDRESS, LFJV2::LBPairCreated::SIGNATURE_HASH),
        );
        protocol_to_address.insert(
            Protocol::LFJV2_2,
            (LFJ_V2_2_DEX_FACTORY_ADDRESS, LFJV2::LBPairCreated::SIGNATURE_HASH),
        );
        protocol_to_address
    }

    pub async fn execute(self, brontes_db_path: String, ctx: CliContext) -> eyre::Result<()> {
        let db_path = get_env_vars()?;

        let max_tasks = self.max_tasks.unwrap_or(num_cpus::get_physical());
        init_thread_pools(max_tasks);

        let libmdbx =
            static_object(load_database(&ctx.task_executor, brontes_db_path, None, None).await?);

        let limiter = self.rate_limit.map(|rate_limit| {
            Arc::new(RateLimiter::direct(Quota::per_second(NonZeroU32::new(rate_limit).unwrap())))
        });

        let tracer = Arc::new(get_tracing_provider_rpc(
            Path::new(&db_path),
            max_tasks as u64,
            ctx.task_executor.clone(),
            limiter,
        ));

        let protocol_to_address = Self::get_protocol_to_address_map();
        let parser =
            static_object(DLogParser::new(libmdbx, tracer.clone(), protocol_to_address).await);

        let start_block = if let Some(s) = self.start_block {
            s
        } else {
            libmdbx.client.max_traced_block().await?
        };
        let end_block =
            if let Some(e) = self.end_block { e } else { parser.get_latest_block_number().await? };

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

        let total_blocks = end_block - start_block + 1;
        let chunk_size = (total_blocks + max_tasks as u64 - 1) / max_tasks as u64; // ceiling division
        let chunks = (start_block..=end_block)
            .chunks(chunk_size as usize)
            .into_iter()
            .map(|mut c| {
                let start = c.next().unwrap();
                let end = c.last().unwrap_or(start);
                (start, end)
            })
            .collect_vec();

        futures::stream::iter(chunks)
            .map(|(start_block, end_block)| {
                let bar = bar.clone();
                ctx.task_executor
                    .spawn_critical_with_graceful_shutdown_signal(
                        "DiscoveryLogs",
                        |shutdown| async move {
                            DiscoveryLogsExecutor::new(
                                start_block,
                                end_block,
                                self.range_size,
                                libmdbx,
                                parser,
                                bar,
                            )
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
