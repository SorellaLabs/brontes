use std::collections::HashMap;

use brontes_types::db::{
    cex::{CexPriceMap, CexQuote},
    redefined_types::{
        malachite::Redefined_Rational,
        primitives::{Redefined_Address, Redefined_Pair},
    },
};
use redefined::{Redefined, RedefinedConvert};
use sorella_db_databases::clickhouse::{self, Row};

use super::{CompressedTable, LibmdbxData};
use crate::libmdbx::CexPrice;

#[derive(Debug, Clone, Row, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CexPriceData {
    pub block_number:     u64,
    pub data: CexPriceMap,
}

impl LibmdbxData<CexPrice> for CexPriceData {
    fn into_key_val(
        &self,
    ) -> (<CexPrice as reth_db::table::Table>::Key, <CexPrice as CompressedTable>::DecompressedValue)
    {
        (self.block_number, self.data.clone())
    }
}

#[derive(
    Debug, Clone, serde::Serialize, rkyv::Serialize, rkyv::Deserialize, rkyv::Archive, Redefined,
)]
#[redefined(CexPriceMap)]
pub struct LibmdbxCexPriceMap(pub HashMap<Redefined_Pair, Vec<LibmdbxCexQuote>>);

#[derive(
    Debug,
    Clone,
    Hash,
    Eq,
    serde::Serialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    Redefined,
)]
#[redefined(CexQuote)]
pub struct LibmdbxCexQuote {
    pub exchange:  Option<String>,
    pub timestamp: u64,
    pub price:     (Redefined_Rational, Redefined_Rational),
    pub token0:    Redefined_Address,
}

impl PartialEq for LibmdbxCexQuote {
    fn eq(&self, other: &Self) -> bool {
        self.clone().to_source().eq(&other.clone().to_source())
    }
}

