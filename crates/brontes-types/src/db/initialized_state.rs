use redefined::self_convert_redefined;
use serde::{Deserialize, Serialize};

use crate::implement_table_value_codecs_with_zc;

/// All these flags are the shift amount of the avaiablilities
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
        should_skip: bool,
        has_dex_price: bool,
        has_traces: bool,
        has_cex_quotes: bool,
        has_cex_trades: bool,
        has_meta: bool,
    ) -> Self {
        let mut this = 0u16;
        if has_dex_price {
            this |= DEX_PRICE_FLAG
        }
        if has_traces {
            this |= TRACE_FLAG
        }
        if has_cex_quotes {
            this |= CEX_QUOTES_FLAG
        }
        if has_cex_trades {
            this |= CEX_TRADES_FLAG
        }
        if has_meta {
            this |= META_FLAG
        }

        Self(this)
    }

    #[inline(always)]
    pub fn set(&mut self, this: u16) {
        self.0 |= this
    }

    #[inline(always)]
    pub fn should_ignore(&self) -> bool {
        // self.0 & SKIP_FLAG != 0
        //kjkj
        false
    }

    #[inline(always)]
    pub fn is_initialized(&self, flag: u16) -> bool {
        (self.0 & flag) == flag
    }

    #[inline(always)]
    pub fn apply_reset_key(&mut self, flag: u16) {
        if self.is_initialized(flag) {
            self.0 ^= flag;
        }
    }
}

self_convert_redefined!(InitializedStateMeta);
implement_table_value_codecs_with_zc!(InitializedStateMeta);
