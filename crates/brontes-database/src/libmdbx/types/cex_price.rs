use std::collections::HashMap;

use brontes_types::{
    db::{
        cex::{CexExchange, CexPriceMap, CexQuote},
        redefined_types::{
            malachite::Redefined_Rational,
            primitives::{Redefined_Address, Redefined_Pair},
        },
    },
    pair::Pair,
};
use redefined::{Redefined, RedefinedConvert};

#[derive(
    Debug, Clone, serde::Serialize, rkyv::Serialize, rkyv::Deserialize, rkyv::Archive, Redefined,
)]
#[redefined(CexPriceMap)]
#[redefined_attr(
    to_source = "CexPriceMap(self.map.to_source())",
    from_source = "LibmdbxCexPriceMap::new(src.0)"
)]
#[archive(check_bytes)]
pub struct LibmdbxCexPriceMap {
    pub map: HashMap<CexExchange, HashMap<Redefined_Pair, LibmdbxCexQuote>>,
}

impl LibmdbxCexPriceMap {
    fn new(map: HashMap<CexExchange, HashMap<Pair, CexQuote>>) -> Self {
        Self { map: HashMap::from_source(map) }
    }
}

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
#[archive(check_bytes)]
#[redefined(CexQuote)]
pub struct LibmdbxCexQuote {
    pub exchange:  CexExchange,
    pub timestamp: u64,
    pub price:     (Redefined_Rational, Redefined_Rational),
    pub token0:    Redefined_Address,
}

impl PartialEq for LibmdbxCexQuote {
    fn eq(&self, other: &Self) -> bool {
        self.clone().to_source().eq(&other.clone().to_source())
    }
}

#[cfg(test)]
pub mod test {
    use std::collections::HashMap;

    use brontes_types::{
        constants::{USDC_ADDRESS, USDT_ADDRESS, WETH_ADDRESS},
        db::cex::{CexExchange, CexQuote},
        pair::Pair,
    };
    use itertools::Itertools;
    use redefined::RedefinedConvert;
    use rkyv::Deserialize;
    use zstd::zstd_safe::WriteBuf;

    use super::LibmdbxCexPriceMap;
    use crate::libmdbx::types::cex_price::ArchivedLibmdbxCexPriceMap;

    #[test]
    fn test_encode_decode() {
        let mut map = HashMap::new();
        let mut inner = HashMap::new();
        let quote = CexQuote { ..Default::default() };
        inner.insert(Pair(WETH_ADDRESS, USDC_ADDRESS), quote.clone());

        inner.insert(Pair(USDC_ADDRESS, USDT_ADDRESS), quote);
        map.insert(CexExchange::Kucoin, inner);

        let copied = LibmdbxCexPriceMap::from_source(brontes_types::db::cex::CexPriceMap(map));
        let ser = rkyv::to_bytes::<_, 255>(&copied).unwrap();
        println!("serialized this");
        let bytes = ser.into_vec();

        let res: &ArchivedLibmdbxCexPriceMap =
            rkyv::check_archived_root::<LibmdbxCexPriceMap>(bytes.as_slice()).unwrap();
        let this: LibmdbxCexPriceMap = res.deserialize(&mut rkyv::Infallible).unwrap();

        println!("deser this {:#?}", this);
    }
}
