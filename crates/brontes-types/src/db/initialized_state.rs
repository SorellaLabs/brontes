use rayon::collections::hash_map;

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
/// [0, 0, 0, should_skip, has_dex_price, has_traces, has_cex_price, has_meta]
pub struct InitializedStateMeta(pub u8);

impl InitializedStateMeta {
    pub fn new(
        should_skip: bool,
        has_dex_price: bool,
        has_traces: bool,
        has_cex_price: bool,
        has_meta: bool,
    ) -> Self {
        let mut this = 0u8;
        if should_skip {
            this |= 0b00010000;
        }
        if has_dex_price {
            this |= 0b00001000
        }
        if has_traces {
            this |= 0b00000100
        }
        if has_cex_price {
            this |= 0b00000010
        }
        if has_meta {
            this |= 0b00000001
        }

        Self(this)
    }

    #[cfg(not(feature = "local"))]
    #[inline(always)]
    pub fn is_init(&self) -> bool {
        self.0 == 0b11 || self.should_ignore()
    }

    #[cfg(feature = "local")]
    #[inline(always)]
    pub fn is_init(&self) -> bool {
        self.0 == 0b111 || self.should_ignore()
    }

    #[inline(always)]
    pub fn has_dex_price(&self) -> bool {
        self.0 >> 3 as bool
    }

    #[inline(always)]
    pub fn should_ignore(&self) -> bool {
        self.0 >> 4 as bool
    }
}

self_convert_redefined!(InitializedStateMeta);
implement_table_value_codecs_with_zc!(InitializedStateMeta);
