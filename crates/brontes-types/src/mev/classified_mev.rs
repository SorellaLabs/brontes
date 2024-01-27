use std::fmt::{self, Debug};

use alloy_primitives::Address;
use colored::Colorize;
use dyn_clone::DynClone;
use indoc::indoc;
use redefined::{self_convert_redefined, RedefinedConvert};
use reth_primitives::B256;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use serde_with::serde_as;
use sorella_db_databases::{
    clickhouse,
    clickhouse::{fixed_string::FixedString, InsertRow, Row},
};
use strum::{Display, EnumIter};

#[allow(unused_imports)]
use crate::{
    display::utils::{display_sandwich, print_mev_type_header},
    normalized_actions::{NormalizedBurn, NormalizedLiquidation, NormalizedMint, NormalizedSwap},
    serde_primitives::vec_fixed_string,
    GasDetails,
};
