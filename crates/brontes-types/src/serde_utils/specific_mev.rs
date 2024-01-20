use serde::{ser::SerializeTuple, Deserialize, Deserializer, Serialize, Serializer};
use sorella_db_databases::clickhouse::InsertRow;

use crate::classified_mev::{
    AtomicBackrun, CexDex, JitLiquidity, JitLiquiditySandwich, Liquidation, MevType, Sandwich,
    SpecificMev,
};

/*
macro_rules! decode_specific {
    ($mev_type:ident, $value:ident, $($mev:ident = $name:ident),+) => {
        match $mev_type {
        $(
            MevType::$mev => Box::new(
                serde_json::from_value::<$name>($value).unwrap()
            ) as SpecificMev,
        )+
        _ => todo!("missing variant")
    }
    };
}
*/

impl Serialize for SpecificMev {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            SpecificMev::Sandwich(sandwich) => sandwich.serialize(serializer),
            SpecificMev::AtomicBackrun(backrun) => backrun.serialize(serializer),
            SpecificMev::JitSandwich(jit_sandwich) => jit_sandwich.serialize(serializer),
            SpecificMev::Jit(jit) => jit.serialize(serializer),
            SpecificMev::CexDex(cex_dex) => cex_dex.serialize(serializer),
            SpecificMev::Liquidation(liquidation) => liquidation.serialize(serializer),
            SpecificMev::Unknown => {
                unimplemented!("attempted to serialize unknown mev: UNIMPLEMENTED")
            }
        }
    }
}

/*
impl<'de> Deserialize<'de> for SpecificMev {
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

impl InsertRow for SpecificMev {
    fn get_column_names(&self) -> &'static [&'static str] {
        match self {
            SpecificMev::Sandwich(sandwich) => sandwich.get_column_names(),
            SpecificMev::AtomicBackrun(backrun) => backrun.get_column_names(),
            SpecificMev::JitSandwich(jit_sandwich) => jit_sandwich.get_column_names(),
            SpecificMev::Jit(jit) => jit.get_column_names(),
            SpecificMev::CexDex(cex_dex) => cex_dex.get_column_names(),
            SpecificMev::Liquidation(liquidation) => liquidation.get_column_names(),
            SpecificMev::Unknown => {
                unimplemented!("attempted to inserted unknown mev into clickhouse: UNIMPLEMENTED")
            }
        }
    }
}
