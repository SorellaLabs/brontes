use redefined::self_convert_redefined;
use serde::{Deserialize, Serialize};

use crate::implement_table_value_codecs_with_zc;

pub const META_FLAG: u8 = 0b1;
pub const CEX_QUOTES_FLAG: u8 = 0b10;
pub const CEX_TRADES_FLAG: u8 = 0b100;
pub const TRACE_FLAG: u8 = 0b1000;
pub const DEX_PRICE_FLAG: u8 = 0b10000;
pub const SKIP_FLAG: u8 = 0b100000;

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
/// there keys are as followed,
/// [0, 0, should_skip, has_dex_price, has_traces,
/// has_cex_trades,has_cex_quotes, has_meta]
pub struct InitializedStateMeta(u8);

impl InitializedStateMeta {
    pub fn new(
        should_skip: bool,
        has_dex_price: bool,
        has_traces: bool,
        has_cex_quotes: bool,
        has_cex_trades: bool,
        has_meta: bool,
    ) -> Self {
        let mut this = 0u8;
        if should_skip {
            this |= SKIP_FLAG;
        }
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
    pub fn set(&mut self, this: u8) {
        self.0 |= this
    }

    #[inline(always)]
    pub fn should_ignore(&self) -> bool {
        self.0 & SKIP_FLAG != 0
    }

    #[inline(always)]
    pub fn is_initialized(&self, flag: u8) -> bool {
        (self.0 & flag) == flag
    }

    #[inline(always)]
    pub fn apply_reset_key(&mut self, flag: u8) {
        if self.is_initialized(flag) {
            self.0 ^= flag;
        }
    }
}

self_convert_redefined!(InitializedStateMeta);
implement_table_value_codecs_with_zc!(InitializedStateMeta);
