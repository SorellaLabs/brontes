use std::{path::Path, sync::Arc};

use alloy_primitives::Address;
use brontes_classifier::get_pendle_v2_sy_pools;
use brontes_database::clickhouse::Clickhouse;
use brontes_types::{db::address_to_protocol_info::ProtocolInfoClickhouse, Protocol};
use clap::Parser;
use db_interfaces::Database;
use eyre::Result;
use tracing::{info, warn};

use crate::cli::{get_env_vars, get_tracing_provider};

/// Insert Pendle V2 SY pools into ClickHouse database
#[derive(Debug, Parser)]
pub struct PendlePoolsCommand {
    /// Skip pools that already exist in the database
    #[arg(long, default_value = "true")]
    pub skip_existing: bool,

    /// Dry run - don't actually insert, just show what would be inserted
    #[arg(long, default_value = "false")]
    pub dry_run: bool,
}

impl PendlePoolsCommand {
    pub async fn execute(&self) -> Result<()> {
        info!("Starting Pendle V2 SY pools insertion...");

        // Initialize environment variables and tracing provider
        let db_path = get_env_vars()?;
        let max_tasks = 10; // Conservative number for this operation

        // Create a dummy task executor for the get_tracing_provider call
        let task_executor =
            brontes_types::BrontesTaskManager::new(tokio::runtime::Handle::current(), false);
        let executor = task_executor.executor();
        tokio::spawn(task_executor);

        let tracer = get_tracing_provider(Path::new(&db_path), max_tasks, executor);

        // Initialize ClickHouse client
        let clickhouse = Clickhouse::new_default(None).await;

        // Fetch Pendle V2 SY pools
        info!("Fetching Pendle V2 SY pools...");
        let pools = get_pendle_v2_sy_pools(&Arc::new(tracer))
            .await
            .map_err(|e| eyre::eyre!("Failed to fetch pools: {}", e))?;
        info!("Found {} Pendle V2 SY pools", pools.len());

        let mut existing_count = 0;
        let mut new_pools = Vec::new();

        if self.skip_existing {
            info!("Checking for existing pools in database...");

            for pool in &pools {
                let pool_exists = self
                    .check_pool_exists(&clickhouse, &pool.pool_address)
                    .await?;

                if pool_exists {
                    existing_count += 1;
                    info!("Pool already exists: {:?}", pool.pool_address);
                } else {
                    new_pools.push(pool);
                }
            }

            info!(
                "Found {} existing pools, {} new pools to insert",
                existing_count,
                new_pools.len()
            );
        } else {
            new_pools = pools.iter().collect();
        }

        if new_pools.is_empty() {
            info!("No new pools to insert");
            return Ok(());
        }

        if self.dry_run {
            info!("Dry run - would insert {} pools:", new_pools.len());
            for pool in &new_pools {
                info!("  Pool: {:?}, Tokens: {:?}", pool.pool_address, pool.tokens);
            }
            return Ok(());
        }

        // Convert to ClickHouse format and insert
        info!("Converting {} pools to ClickHouse format...", new_pools.len());
        let clickhouse_pools: Vec<ProtocolInfoClickhouse> = new_pools
            .into_iter()
            .map(|pool| {
                ProtocolInfoClickhouse::new(
                    pool.trace_index, // Using trace_index as init_block (0 for fetched pools)
                    pool.pool_address,
                    &pool.tokens,
                    None, // No curve_lp_token for Pendle pools
                    pool.protocol,
                )
            })
            .collect();

        info!("Inserting {} pools into ClickHouse...", clickhouse_pools.len());

        // Insert pools into ClickHouse
        for pool in &clickhouse_pools {
            if let Err(e) = clickhouse
                .insert_pool(
                    pool.init_block,
                    pool.address.to_string().parse::<Address>()?,
                    &pool
                        .tokens
                        .iter()
                        .map(|t| t.to_string().parse::<Address>())
                        .collect::<Result<Vec<_>, _>>()?,
                    pool.curve_lp_token
                        .as_ref()
                        .map(|t| t.to_string().parse::<Address>())
                        .transpose()?,
                    Protocol::PendleV2,
                )
                .await
            {
                warn!("Failed to insert pool {:?}: {}", pool.address, e);
            } else {
                info!("Inserted pool: {:?}", pool.address);
            }
        }

        info!("Successfully processed {} Pendle V2 SY pools", clickhouse_pools.len());
        Ok(())
    }

    async fn check_pool_exists(
        &self,
        clickhouse: &Clickhouse,
        pool_address: &Address,
    ) -> Result<bool> {
        let query = format!(
            "SELECT COUNT(*) as count FROM ethereum.pools WHERE address = '{:?}'",
            pool_address
        );

        #[derive(serde::Deserialize, clickhouse::Row)]
        struct CountResult {
            count: u64,
        }

        let result: Vec<CountResult> = clickhouse
            .client
            .query_many(&query, &())
            .await
            .map_err(|e| eyre::eyre!("ClickHouse query failed: {}", e))?;

        Ok(result.first().map(|r| r.count > 0).unwrap_or(false))
    }
}
