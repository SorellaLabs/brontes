// use brontes_types::{
//     db::{
//         address_to_protocol_info::ProtocolInfoPostgres, block_analysis::BlockAnalysis,
//         dex::DexQuotesWithBlockNumber, normalized_actions::TransactionRoot,
//         token_info::TokenInfoWithAddress, DbDataWithRunId, RunId,
//     },
//     mev::*,
// };
// use db_interfaces::{postgres_dbms, remote_postgres_table};

pub struct BrontesPostgresTables;


// clickhouse_dbms!(
//     BrontesPostgresTables,
//     "eth_cluster0",
//     [
//         BrontesDex_Price_Mapping,
//         BrontesBlock_Analysis,
//         MevMev_Blocks,
//         MevBundle_Header,
//         MevSearcher_Tx,
//         MevCex_Dex,
//         MevLiquidations,
//         MevJit_Sandwich,
//         MevJit,
//         MevSandwiches,
//         MevAtomic_Arbs,
//         BrontesToken_Info,
//         EthereumPools,
//         BrontesTree,
//         BrontesRun_Id
//     ]
// );

// impl BrontesPostgresTables {
//     pub const fn is_big(&self) -> bool {
//         matches!(
//             self,
//             BrontesPostgresTables::BrontesDex_Price_Mapping
//                 | BrontesPostgresTables::BrontesTree
//         )
//     }
// }

// remote_clickhouse_table!(
//     BrontesPostgresTables,
//     [Brontes, Dex_Price_Mapping],
//     DexQuotesWithBlockNumber,
//     "crates/brontes-database/brontes-db/src/clickhouse/tables/"
// );

// remote_clickhouse_table!(
//     BrontesClickhouseTables,
//     [Brontes, Block_Analysis],
//     DbDataWithRunId<BlockAnalysis>,
//     "crates/brontes-database/brontes-db/src/clickhouse/tables/"
// );

// remote_clickhouse_table!(
//     BrontesClickhouseTables,
//     [Mev, Mev_Blocks],
//     DbDataWithRunId<MevBlock>,
//     "crates/brontes-database/brontes-db/src/clickhouse/tables/"
// );

// remote_clickhouse_table!(
//     BrontesClickhouseTables,
//     [Mev, Bundle_Header],
//     DbDataWithRunId<BundleHeader>,
//     "crates/brontes-database/brontes-db/src/clickhouse/tables/"
// );

// remote_clickhouse_table!(
//     BrontesClickhouseTables,
//     [Mev, Searcher_Tx],
//     DbDataWithRunId<SearcherTx>,
//     "crates/brontes-database/brontes-db/src/clickhouse/tables/"
// );

// remote_clickhouse_table!(
//     BrontesClickhouseTables,
//     [Mev, Cex_Dex],
//     DbDataWithRunId<CexDex>,
//     "crates/brontes-database/brontes-db/src/clickhouse/tables/"
// );

// remote_clickhouse_table!(
//     BrontesClickhouseTables,
//     [Mev, Liquidations],
//     DbDataWithRunId<Liquidation>,
//     "crates/brontes-database/brontes-db/src/clickhouse/tables/"
// );

// remote_clickhouse_table!(
//     BrontesClickhouseTables,
//     [Mev, Jit_Sandwich],
//     DbDataWithRunId<JitLiquiditySandwich>,
//     "crates/brontes-database/brontes-db/src/clickhouse/tables/"
// );

// remote_clickhouse_table!(
//     BrontesClickhouseTables,
//     [Mev, Jit],
//     DbDataWithRunId<JitLiquidity>,
//     "crates/brontes-database/brontes-db/src/clickhouse/tables/"
// );

// remote_clickhouse_table!(
//     BrontesClickhouseTables,
//     [Mev, Sandwiches],
//     DbDataWithRunId<Sandwich>,
//     "crates/brontes-database/brontes-db/src/clickhouse/tables/"
// );

// remote_clickhouse_table!(
//     BrontesClickhouseTables,
//     [Mev, Atomic_Arbs],
//     DbDataWithRunId<AtomicArb>,
//     "crates/brontes-database/brontes-db/src/clickhouse/tables/"
// );

// remote_clickhouse_table!(
//     BrontesClickhouseTables,
//     [Brontes, Token_Info],
//     TokenInfoWithAddress,
//     "crates/brontes-database/brontes-db/src/clickhouse/tables/"
// );

// remote_clickhouse_table!(
//     BrontesClickhouseTables,
//     [Ethereum, Pools],
//     ProtocolInfoClickhouse,
//     "crates/brontes-database/brontes-db/src/clickhouse/tables/"
// );

// remote_clickhouse_table!(
//     BrontesClickhouseTables,
//     [Brontes, Tree],
//     DbDataWithRunId<TransactionRoot>,
//     "crates/brontes-database/brontes-db/src/clickhouse/tables/"
// );

// remote_clickhouse_table!(
//     BrontesClickhouseTables,
//     [Brontes, Run_Id],
//     RunId,
//     "crates/brontes-database/brontes-db/src/clickhouse/tables/"
// );

pub struct BrontesPostgresData {
    pub data:         BrontesPostgresTableDataTypes,
    pub force_insert: bool,
}

// macro_rules! db_types {
//     ($(($db_type:ident, $db_table:ident, $t:tt)),*) => {
//         db_types!(enum_s {}, $($db_type, $t,)*);

//         paste::paste! {
//             impl BrontesPostgresTableDataTypes {
//                 pub fn get_db_enum(&self) -> BrontesPostgresTables {
//                     match self {
//                         $(
//                             BrontesPostgresTableDataTypes::$db_type(_) =>
//                                 BrontesPostgresTables::$db_table,
//                         )*
//                     }
//                 }
//             }
//         }

//         $(
//             db_types!($db_type, $t);

//         )*
//     };
//     ($db_type:ident, true) => {
//             impl From<($db_type, bool, u64)> for BrontesPostgresData {
//                 fn from(value: ($db_type, bool, u64)) ->BrontesPostgresData {
//                     BrontesPostgresData {
//                         data: BrontesPostgresTableDataTypes::$db_type(Box::new(
//                                       DbDataWithRunId {
//                                           table: value.0,
//                                           run_id: value.2
//                                       }
//                                       )),
//                         force_insert: value.1
//                     }
//                 }
//             }

//     };
//     ($db_type:ident, false) => {
//         impl From<($db_type, bool)> for BrontesPostgresData {
//             fn from(value: ($db_type, bool)) ->BrontesPostgresData {
//                 BrontesPostgresData {
//                     data: BrontesPostgresTableDataTypes::$db_type(Box::new(value.0)),
//                     force_insert: value.1
//                 }
//             }
//         }
//     };
//     (enum_s  {$($acc:tt)* }, $db_type:ident, true, $($tail:tt)*) => {
//         db_types!(enum_s {
//             $($acc)*
//             $db_type(Box<DbDataWithRunId<$db_type>>),
//         }, $($tail)*);
//     };
//     (enum_s {$($acc:tt)* }, $db_type:ident, false, $($tail:tt)*) => {
//         db_types!(enum_s {
//             $($acc)*
//             $db_type(Box<$db_type>),
//         }, $($tail)*);
//     };
//     (enum_s {$($acc:tt)*},$(,)*) => {
//         #[derive(Debug, Clone, serde::Serialize)]
//         #[serde(untagged)]
//         #[allow(clippy::large_enum_variant)]
//         pub enum BrontesPostgresTableDataTypes {
//             $($acc)*
//         }
//     }
// }

// TODO(tim) - fix this!
pub enum BrontesPostgresTableDataTypes {}

// db_types!(
//     (DexQuotesWithBlockNumber, BrontesDex_Price_Mapping, false),
//     (MevBlock, MevMev_Blocks, true),
//     (BundleHeader, MevBundle_Header, true),
//     (SearcherTx, MevSearcher_Tx, true),
//     (CexDex, MevCex_Dex, true),
//     (Liquidation, MevLiquidations, true),
//     (JitLiquiditySandwich, MevJit_Sandwich, true),
//     (JitLiquidity, MevJit, true),
//     (Sandwich, MevSandwiches, true),
//     (AtomicArb, MevAtomic_Arbs, true),
//     (TokenInfoWithAddress, BrontesToken_Info, false),
//     (ProtocolInfoPostgres, EthereumPools, false),
//     (TransactionRoot, BrontesTree, true),
//     (BlockAnalysis, BrontesBlock_Analysis, true),
//     (RunId, BrontesRun_Id, false)
// );
