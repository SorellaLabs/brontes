pub mod dex_quote {
    use std::str::FromStr;

    use alloy_primitives::Address;
    use itertools::Itertools;
    use malachite::{Natural, Rational};
    use serde::{
        de::{Deserialize, Deserializer},
        Serialize, Serializer,
    };

    use crate::{db::dex::DexPrices, pair::Pair, FastHashMap};

    type DexPriceQuotesVec = Vec<((String, String), ((Vec<u64>, Vec<u64>), (Vec<u64>, Vec<u64>)))>;

    #[allow(dead_code)]
    pub fn serialize<S>(
        value: &Option<FastHashMap<Pair, DexPrices>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let to_ser: DexPriceQuotesVec = if let Some(quotes) = value {
            quotes
                .iter()
                .map(|(pair, dex_price)| {
                    (
                        (format!("{:?}", pair.0), format!("{:?}", pair.1)),
                        (
                            (
                                dex_price.pre_state.numerator_ref().to_limbs_asc(),
                                dex_price.pre_state.denominator_ref().to_limbs_asc(),
                            ),
                            (
                                dex_price.post_state.numerator_ref().to_limbs_asc(),
                                dex_price.post_state.denominator_ref().to_limbs_asc(),
                            ),
                        ),
                    )
                })
                .collect_vec()
        } else {
            vec![]
        };

        to_ser.serialize(serializer)
    }

    #[allow(dead_code)]
    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<Option<FastHashMap<Pair, DexPrices>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let des: DexPriceQuotesVec = Deserialize::deserialize(deserializer)?;

        if des.is_empty() {
            return Ok(None)
        }

        let val = des
            .into_iter()
            .map(|((pair0, pair1), ((pre_num, pre_den), (post_num, post_den)))| {
                (
                    Pair(Address::from_str(&pair0).unwrap(), Address::from_str(&pair1).unwrap())
                        .ordered(),
                    DexPrices {
                        pre_state:    Rational::from_naturals(
                            Natural::from_owned_limbs_asc(pre_num),
                            Natural::from_owned_limbs_asc(pre_den),
                        ),
                        post_state:   Rational::from_naturals(
                            Natural::from_owned_limbs_asc(post_num),
                            Natural::from_owned_limbs_asc(post_den),
                        ),
                        goes_through: Pair::default(),
                        is_transfer:  false,
                    },
                )
            })
            .collect::<FastHashMap<Pair, DexPrices>>();

        Ok(Some(val))
    }
}
