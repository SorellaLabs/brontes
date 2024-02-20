pub mod dex_quote {
    use std::{collections::HashMap, str::FromStr};

    use alloy_primitives::{Address, U256};
    use itertools::Itertools;
    use malachite::{Natural, Rational};
    use serde::{
        de::{Deserialize, Deserializer},
        Serialize, Serializer,
    };

    use crate::{db::dex::DexPrices, pair::Pair};

    type DexPriceQuotesVec = Vec<(
        (String, String),
        (([u8; 32], [u8; 32]), ([u8; 32], [u8; 32])),
    )>;

    #[allow(dead_code)]
    pub fn serialize<S>(
        value: &Option<HashMap<Pair, DexPrices>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let to_ser: DexPriceQuotesVec = if let Some(quotes) = value {
            quotes
                .into_iter()
                .map(|(pair, dex_price)| {
                    (
                        (format!("{:?}", pair.0), format!("{:?}", pair.1)),
                        (
                            (
                                U256::from_limbs_slice(
                                    &dex_price.pre_state.numerator_ref().to_limbs_asc(),
                                )
                                .to_le_bytes(),
                                U256::from_limbs_slice(
                                    &dex_price.pre_state.denominator_ref().to_limbs_asc(),
                                )
                                .to_le_bytes(),
                            ),
                            (
                                U256::from_limbs_slice(
                                    &dex_price.post_state.numerator_ref().to_limbs_asc(),
                                )
                                .to_le_bytes(),
                                U256::from_limbs_slice(
                                    &dex_price.post_state.denominator_ref().to_limbs_asc(),
                                )
                                .to_le_bytes(),
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
    ) -> Result<Option<HashMap<Pair, DexPrices>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let des: DexPriceQuotesVec = Deserialize::deserialize(deserializer)?;

        if des.is_empty() {
            return Ok(None);
        }

        let val = des
            .into_iter()
            .map(
                |((pair0, pair1), ((pre_num, pre_den), (post_num, post_den)))| {
                    (
                        Pair(
                            Address::from_str(&pair0).unwrap(),
                            Address::from_str(&pair1).unwrap(),
                        )
                        .ordered(),
                        DexPrices {
                            pre_state: Rational::from_naturals(
                                Natural::from_limbs_asc(&U256::from_le_bytes(pre_num).into_limbs()),
                                Natural::from_limbs_asc(&U256::from_le_bytes(pre_den).into_limbs()),
                            ),
                            post_state: Rational::from_naturals(
                                Natural::from_limbs_asc(
                                    &U256::from_le_bytes(post_num).into_limbs(),
                                ),
                                Natural::from_limbs_asc(
                                    &U256::from_le_bytes(post_den).into_limbs(),
                                ),
                            ),
                        },
                    )
                },
            )
            .collect::<HashMap<Pair, DexPrices>>();

        Ok(Some(val))
    }
}
