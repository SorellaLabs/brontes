use alloy_primitives::Address;
use brontes_inspect::DynMevInspector;
use brontes_types::db::{cex::CexExchange, traits::LibmdbxReader};
use clap::Args;

/// Implement this trait to extend brontes with your own [Inspector](s).
pub trait InspectorCliExt {
    /// Override this to initialize your custom [Inspector](s).
    fn init_mev_inspectors<DB: LibmdbxReader>(
        &self,
        quote_token: Address,
        db: &'static DB,
        cex_exchanges: &[CexExchange],
    ) -> Vec<DynMevInspector>;
}

/// Noop impl for [InspectorCliExt].
#[derive(Debug, Clone, Copy, Default, Args)]
pub struct NoopInspectorCliExt;
impl InspectorCliExt for NoopInspectorCliExt {
    fn init_mev_inspectors<DB: LibmdbxReader>(
        &self,
        _quote_token: Address,
        _db: &'static DB,
        _cex_exchanges: &[CexExchange],
    ) -> Vec<DynMevInspector> {
        vec![]
    }
}
