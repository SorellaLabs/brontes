use redefined::self_convert_redefined;
use serde::{Deserialize, Serialize};

use crate::implement_table_value_codecs_with_zc;

// All these flags are the shift amount of the availabilities
pub const META_FLAG: u16 = 0;
pub const CEX_QUOTES_FLAG: u16 = 2;
pub const CEX_TRADES_FLAG: u16 = 4;
pub const TRACE_FLAG: u16 = 6;
pub const DEX_PRICE_FLAG: u16 = 8;

/// Data not present, availability unknown
pub const DATA_NOT_PRESENT_UNKNOWN: u16 = 0b00;
///  Data not present and not available
pub const DATA_NOT_PRESENT_NOT_AVAILABLE: u16 = 0b01;
/// Data not present but available (i.e., confirmed empty, not present in
/// clickhouse)
pub const DATA_NOT_PRESENT_BUT_AVAILABLE: u16 = 0b10;
pub const DATA_PRESENT: u16 = 0b11;

#[derive(
    Debug,
    Default,
    PartialEq,
    Clone,
    Copy,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
)]
#[repr(transparent)]
/// InitializedState allows for us to mark up to 8 fields in
/// the database as initialized
pub struct InitializedStateMeta(u16);

impl InitializedStateMeta {
    pub fn new(
        has_dex_price: u16,
        has_traces: u16,
        has_cex_quotes: u16,
        has_cex_trades: u16,
        has_meta: u16,
    ) -> Self {
        let mut this = 0u16;
        this |= has_dex_price << DEX_PRICE_FLAG;
        this |= has_traces << TRACE_FLAG;
        this |= has_cex_quotes << CEX_QUOTES_FLAG;
        this |= has_cex_trades << CEX_TRADES_FLAG;
        this |= has_meta << META_FLAG;

        Self(this)
    }

    #[inline(always)]
    pub fn merge(self, other: Self) -> InitializedStateMeta {
        InitializedStateMeta(self.0 | other.0)
    }

    #[inline(always)]
    pub fn set(&mut self, this: u16, availability: u16) {
        // reset the data at the given offset
        self.0 &= u16::MAX ^ (DATA_PRESENT << this);
        // set availability
        self.0 |= availability << this
    }

    #[inline(always)]
    pub fn is_initialized(&self, flag: u16) -> bool {
        (self.0 & (DATA_PRESENT << flag)) == (DATA_PRESENT << flag)
            || (self.0 & (DATA_NOT_PRESENT_NOT_AVAILABLE << flag))
                == (DATA_NOT_PRESENT_NOT_AVAILABLE << flag)
    }

    #[inline(always)]
    pub fn apply_reset_key(&mut self, flag: u16) {
        if self.is_initialized(flag) {
            // reset the data at the given offset
            self.0 &= u16::MAX ^ (DATA_PRESENT << flag);
        }
    }
}

self_convert_redefined!(InitializedStateMeta);
implement_table_value_codecs_with_zc!(InitializedStateMeta);
