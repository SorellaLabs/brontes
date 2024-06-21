use clap::Parser;
use reqwest::Url;

use crate::runner::CliContext;

#[derive(Debug, Parser)]
pub struct R2Uploader {
    /// endpoint url
    #[arg(long, short, default_value = "https://pub-e19b2b40b9c14ec3836e65c2c04590ec.r2.dev")]
    pub endpoint: Url,
}

impl R2Uploader {
    pub async fn execute(self, _: CliContext) -> eyre::Result<()> {
        Ok(())
    }
}
