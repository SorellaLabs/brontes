use clap::Parser;

#[derive(Debug)]
pub struct TraceArg {
    #[arg(long, short)]
    pub block_num: u64,
}

impl TraceArg {
    pub async fn execute(self) -> eyre::Result<()> {
        brontes_core::store_traces_for_block(self.block_num).await;
        Ok(())
    }
}
