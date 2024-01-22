use serde::{Serialize, Serializer};
use sorella_db_databases::clickhouse::InsertRow;

use crate::classified_mev::BundleData;

/*
macro_rules! decode_specific {
    ($mev_type:ident, $value:ident, $($mev:ident = $name:ident),+) => {
        match $mev_type {
        $(
            MevType::$mev => Box::new(
                serde_json::from_value::<$name>($value).unwrap()
            ) as BundleData,
        )+
        _ => todo!("missing variant")
    }
    };
}
*/

impl Serialize for BundleData {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            BundleData::Sandwich(sandwich) => sandwich.serialize(serializer),
            BundleData::AtomicBackrun(backrun) => backrun.serialize(serializer),
            BundleData::JitSandwich(jit_sandwich) => jit_sandwich.serialize(serializer),
            BundleData::Jit(jit) => jit.serialize(serializer),
            BundleData::CexDex(cex_dex) => cex_dex.serialize(serializer),
            BundleData::Liquidation(liquidation) => liquidation.serialize(serializer),
            BundleData::Unknown => {
                unimplemented!("attempted to serialize unknown mev: UNIMPLEMENTED")
            }
        }
    }
}

/*
impl<'de> Deserialize<'de> for BundleData {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let (mev_type, val) = <(MevType, serde_json::Value)>::deserialize(deserializer)?;

        Ok(decode_specific!(
            mev_type,
            val,
            Backrun = AtomicBackrun,
            Jit = JitLiquidity,
            JitSandwich = JitLiquiditySandwich,
            Sandwich = Sandwich,
            CexDex = CexDex,
            Liquidation = Liquidation
        ))
    }
}
*/

impl InsertRow for BundleData {
    fn get_column_names(&self) -> &'static [&'static str] {
        match self {
            BundleData::Sandwich(sandwich) => sandwich.get_column_names(),
            BundleData::AtomicBackrun(backrun) => backrun.get_column_names(),
            BundleData::JitSandwich(jit_sandwich) => jit_sandwich.get_column_names(),
            BundleData::Jit(jit) => jit.get_column_names(),
            BundleData::CexDex(cex_dex) => cex_dex.get_column_names(),
            BundleData::Liquidation(liquidation) => liquidation.get_column_names(),
            BundleData::Unknown => {
                unimplemented!("attempted to inserted unknown mev into clickhouse: UNIMPLEMENTED")
            }
        }
    }
}
