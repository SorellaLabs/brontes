use serde::{ser::SerializeTuple, Deserialize, Deserializer, Serialize, Serializer};
use sorella_db_databases::clickhouse::InsertRow;

use crate::classified_mev::{
    AtomicBackrun, CexDex, JitLiquidity, JitLiquiditySandwich, Liquidation, MevType, Sandwich,
    SpecificMev,
};

macro_rules! decode_specific {
    ($mev_type:ident, $value:ident, $($mev:ident = $name:ident),+) => {
        match $mev_type {
        $(
            MevType::$mev => Box::new(
                serde_json::from_value::<$name>($value).unwrap()
            ) as Box<dyn SpecificMev>,
        )+
        _ => todo!("missing variant")
    }
    };
}

impl Serialize for Box<dyn SpecificMev> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut tup = serializer.serialize_tuple(2)?;
        let mev_type = self.mev_type();
        tup.serialize_element(&mev_type)?;
        let any = self.clone().into_any();

        match mev_type {
            MevType::Sandwich => {
                let this = any.downcast_ref::<Sandwich>().unwrap();
                tup.serialize_element(&this)?;
            }
            MevType::Backrun => {
                let this = any.downcast_ref::<AtomicBackrun>().unwrap();
                tup.serialize_element(&this)?;
            }
            MevType::JitSandwich => {
                let this = any.downcast_ref::<JitLiquiditySandwich>().unwrap();
                tup.serialize_element(&this)?;
            }
            MevType::Jit => {
                let this = any.downcast_ref::<JitLiquidity>().unwrap();
                tup.serialize_element(&this)?;
            }
            MevType::CexDex => {
                let this = any.downcast_ref::<CexDex>().unwrap();
                tup.serialize_element(&this)?;
            }
            MevType::Liquidation => {
                let this = any.downcast_ref::<Liquidation>().unwrap();
                tup.serialize_element(&this)?;
            }
            MevType::Unknown => unimplemented!("none yet"),
        }
        tup.end()
    }
}

impl<'de> Deserialize<'de> for Box<dyn SpecificMev> {
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

impl InsertRow for Box<dyn SpecificMev> {
    fn get_column_names(&self) -> &'static [&'static str] {
        (**self).get_column_names()
    }
}
