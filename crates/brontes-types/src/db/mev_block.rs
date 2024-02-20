use redefined::Redefined;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{Deserialize, Serialize};

use crate::{implement_table_value_codecs_with_zc, mev::*};

#[derive(Debug, Serialize, PartialEq, Deserialize, Clone, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct MevBlockWithClassified {
    pub block: MevBlock,
    pub mev: Vec<Bundle>,
}

implement_table_value_codecs_with_zc!(MevBlockWithClassifiedRedefined);
