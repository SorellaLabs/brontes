use brontes_types::db::{
    address_to_tokens::PoolTokens, redefined_types::primitives::Redefined_Address,
};
use redefined::{Redefined, RedefinedConvert};

#[derive(
    Debug,
    PartialEq,
    Clone,
    serde::Serialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    Redefined,
)]
#[archive(check_bytes)]
#[redefined(PoolTokens)]
pub struct LibmdbxPoolTokens {
    pub token0:     Redefined_Address,
    pub token1:     Redefined_Address,
    pub token2:     Option<Redefined_Address>,
    pub token3:     Option<Redefined_Address>,
    pub token4:     Option<Redefined_Address>,
    pub init_block: u64,
}
