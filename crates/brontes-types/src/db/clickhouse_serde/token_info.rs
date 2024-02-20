pub mod token_info {

    use serde::de::{Deserialize, Deserializer};

    use crate::db::token_info::TokenInfo;

    #[allow(dead_code)]
    pub fn deserialize<'de, D>(deserializer: D) -> Result<TokenInfo, D::Error>
    where
        D: Deserializer<'de>,
    {
        let (decimals, symbol): (u8, String) = Deserialize::deserialize(deserializer)?;

        Ok(TokenInfo { decimals, symbol })
    }
}
