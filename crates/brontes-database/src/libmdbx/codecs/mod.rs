use alloy_rlp::{Decodable, Encodable};
use brontes_types::{
    classified_mev::{
        AtomicBackrun, CexDex, ClassifiedMev, JitLiquidity, JitLiquiditySandwich, Liquidation,
        MevBlock, MevType, PriceKind, Sandwich, SpecificMev,
    },
    db::{
        mev_block::MevBlockWithClassified,
        redefined_types::primitives::{Redefined_Address, Redefined_FixedBytes, Redefined_Uint},
    },
    normalized_actions::{NormalizedBurn, NormalizedLiquidation, NormalizedMint, NormalizedSwap},
    tree::GasDetails,
};
use bytes::BufMut;
use redefined::{Redefined, RedefinedConvert};
use reth_db::{
    table::{Compress, Decompress},
    DatabaseError,
};
use sorella_db_databases::clickhouse::{self, Row};
