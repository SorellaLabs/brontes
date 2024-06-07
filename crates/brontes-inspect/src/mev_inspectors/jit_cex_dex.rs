use brontes_core::LibmdbxReader;

use crate::{cex_dex_markout::CexDexMarkoutInspector, jit::JitInspector};

/// jit cex dex happens when two things are present.
/// 1) a cex dex arb on a pool
/// 2) a user swap on the pool where the volume
/// is greater than the amount the market marker would
/// fill to move the pool to the true price.
///
/// when this occurs market makers add liquidity to 
/// the pool at a price that is worse than true price and get filled 
/// more volume than they would otherwise from the user swapping through.
pub struct JitCexDex<'db, DB: LibmdbxReader> {
    cex_dex: CexDexMarkoutInspector<'db, DB>,
    jit:     JitInspector<'db, DB>,
}
