#![allow(unexpected_cfgs)]

use malachite::{Natural, Rational};
use redefined::{redefined_remote, Redefined};
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::Serialize;

// Rational
redefined_remote!(
    #[derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        Hash,
        Serialize,
        rSerialize,
        rDeserialize,
        Archive,
    )]
    [Rational] : "malachite-q"
);

// Natural
redefined_remote!(
    #[derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        Hash,
        Serialize,
        rSerialize,
        rDeserialize,
        Archive,
    )]
    [Natural] : "malachite-nz"
);

// InnerNatural
redefined_remote!(
    #[derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        Hash,
        Serialize,
        rSerialize,
        rDeserialize,
        Archive,
    )]
    [InnerNatural] : "malachite-nz" : no_impl
);

pub type LimbRedefined = u64;
