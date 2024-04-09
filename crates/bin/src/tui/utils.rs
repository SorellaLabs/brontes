use std::path::PathBuf;

use color_eyre::eyre::Result;
use directories::ProjectDirs;
use lazy_static::lazy_static;
use tracing::error;
use tracing_error::ErrorLayer;
use tracing_subscriber::{
    prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt, Layer,
};
use brontes_types::hasher::FastHashSet;

pub fn get_config_dir() -> PathBuf {
    let directory = PathBuf::from(".").join("config");
    tracing::info!("Config directory: {:?}", directory);
    directory
}

#[macro_export]
macro_rules! get_symbols_from_transaction_accounting {
    ($data:expr) => {{
        use std::collections::HashSet;

        use brontes_types::db::token_info::TokenInfoWithAddress;

        let mut symbols = FastHashSet::default();
        $data.iter()
            .flat_map(|transaction| &transaction.address_deltas)
            .flat_map(|address_delta| &address_delta.token_deltas)
            .for_each(|token_delta| {
                symbols.insert(token_delta.token.inner.symbol.clone());
            });
        
        let unique_symbols = symbols.into_iter().collect::<Vec<String>>().join(", ");
    }};
}
