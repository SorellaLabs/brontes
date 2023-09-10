pub(crate) mod as_rational {
    use malachite::Rational;
    use serde::{self, Deserialize, Deserializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Rational, D::Error>
    where
        D: Deserializer<'de>
    {
        let val = f64::deserialize(deserializer)?;
        Ok(Rational::try_from(val).unwrap())
    }
}
