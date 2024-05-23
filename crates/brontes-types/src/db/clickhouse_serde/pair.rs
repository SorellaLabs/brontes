pub mod pair_ser {

    use serde::{Serialize, Serializer};

    use crate::pair::Pair;

    #[allow(dead_code)]
    pub fn serialize<S>(value: &Pair, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        (format!("{:#?}", value.0), format!("{:#?}", value.1)).serialize(serializer)
    }
}

pub mod addr_ser {

    use alloy_primitives::Address;
    use serde::{Serialize, Serializer};

    #[allow(dead_code)]
    pub fn serialize<S>(value: &Address, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        format!("{:#?}", value).serialize(serializer)
    }
}
