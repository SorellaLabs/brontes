pub mod bench;
pub mod bincode;
pub mod rlp;
pub mod zero_copy;

use std::{env, sync::Arc};

use alloy_primitives::{Address, TxHash, U256};
use arrow::{
    array::{Array, BinaryArray, UInt64Array},
    datatypes::{DataType, Field, Schema},
    record_batch::RecordBatch,
};
use brontes_database_libmdbx::types::utils::{option_address, u256};
use parquet::data_type::AsBytes;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use sorella_db_databases::{clickhouse, Row};

use crate::{
    libmdbx_impl::LibmdbxBench,
    setup::{tables::BenchTables, utils::ToRecordBatch},
};

const METADATA_PARQUET_FILE: &str = "benchmarks/metadata/data/metadata.parquet";
const METADATA_LIBMDBX_DIR: &str = "benchmarks/metadata/data/db";

pub const METADATA_QUERY: &str = "
SELECT
    block_number,
    block_hash,
    block_timestamp,
    relay_timestamp,
    p2p_timestamp,
    proposer_fee_recipient,
    proposer_mev_reward,
    mempool_flow
FROM brontes.metadata
WHERE block_number >= 17000000 AND block_number < 18000000";
// WHERE block_number >= 17500000 AND block_number < 18000000 ";

// Define a schema for the Metadata struct
pub fn metadata_schema() -> Schema {
    Schema::new(vec![
        Field::new("block_number", DataType::UInt64, false),
        Field::new("block_hash", DataType::Binary, false),
        Field::new("block_timestamp", DataType::UInt64, false),
        Field::new("relay_timestamp", DataType::UInt64, true),
        Field::new("p2p_timestamp", DataType::UInt64, true),
        Field::new("proposer_fee_recipient", DataType::Binary, true),
        Field::new("proposer_mev_reward", DataType::Binary, true),
        Field::new("mempool_flow", DataType::Binary, false),
    ])
}

#[serde_as]
#[derive(Row, Debug, Default, Clone, Serialize, Deserialize)]
pub struct MetadataBench {
    pub block_number:           u64,
    #[serde(with = "u256")]
    pub block_hash:             U256,
    pub block_timestamp:        u64,
    pub relay_timestamp:        Option<u64>,
    pub p2p_timestamp:          Option<u64>,
    #[serde(with = "option_address")]
    pub proposer_fee_recipient: Option<Address>,
    pub proposer_mev_reward:    Option<u128>,
    #[serde_as(as = "Vec<DisplayFromStr>")]
    pub mempool_flow:           Vec<TxHash>,
}

impl From<RecordBatch> for MetadataBench {
    fn from(batch: RecordBatch) -> Self {
        let block_number_column = batch
            .column(0)
            .as_any()
            .downcast_ref::<UInt64Array>()
            .unwrap();
        let block_hash_column = batch
            .column(1)
            .as_any()
            .downcast_ref::<BinaryArray>()
            .unwrap();
        let block_timestamp_column = batch
            .column(2)
            .as_any()
            .downcast_ref::<UInt64Array>()
            .unwrap();
        let relay_timestamp_column = batch
            .column(3)
            .as_any()
            .downcast_ref::<UInt64Array>()
            .unwrap();
        let p2p_timestamp_column = batch
            .column(4)
            .as_any()
            .downcast_ref::<UInt64Array>()
            .unwrap();
        let proposer_fee_recipient_column = batch
            .column(5)
            .as_any()
            .downcast_ref::<BinaryArray>()
            .unwrap();
        let proposer_mev_reward_column = batch
            .column(6)
            .as_any()
            .downcast_ref::<BinaryArray>()
            .unwrap();
        let mempool_flow_column = batch
            .column(7)
            .as_any()
            .downcast_ref::<BinaryArray>()
            .unwrap();

        MetadataBench {
            block_number:           block_number_column.value(0),
            block_hash:             U256::from_be_slice(block_hash_column.value(0)),
            block_timestamp:        block_timestamp_column.value(0),
            relay_timestamp:        relay_timestamp_column
                .is_null(0)
                .then(|| relay_timestamp_column.value(0)),
            p2p_timestamp:          p2p_timestamp_column
                .is_null(0)
                .then(|| p2p_timestamp_column.value(0)),
            proposer_fee_recipient: if proposer_fee_recipient_column.is_null(0) {
                None
            } else {
                Some(Address::from_slice(proposer_fee_recipient_column.value(0)))
            },
            proposer_mev_reward:    if proposer_mev_reward_column.is_null(0) {
                None
            } else {
                let mut buf = [0u8; 16];
                buf.copy_from_slice(proposer_mev_reward_column.value(0));
                Some(u128::from_be_bytes(buf))
            },
            mempool_flow:           mempool_flow_column
                .values()
                .chunks(32)
                .map(|chunk| TxHash::from_slice(chunk))
                .collect(),
        }
    }
}

impl ToRecordBatch for MetadataBench {
    fn into_record_batch(rows: Vec<Self>) -> RecordBatch {
        let block_numbers = rows.iter().map(|row| row.block_number).collect::<Vec<_>>();
        let block_number_array = UInt64Array::from(block_numbers);

        let block_hashes = rows
            .iter()
            .map(|row| row.block_hash.to_be_bytes::<32>())
            .collect::<Vec<_>>();
        let block_hash_array = BinaryArray::from_vec(
            block_hashes
                .iter()
                .map(|v| v.as_bytes())
                .collect::<Vec<_>>(),
        );

        let block_timestamps = rows
            .iter()
            .map(|row| row.block_timestamp)
            .collect::<Vec<_>>();
        let block_timestamp_array = UInt64Array::from(block_timestamps);

        let relay_timestamps = rows
            .iter()
            .map(|row| row.relay_timestamp)
            .collect::<Vec<_>>();
        let relay_timestamp_array = UInt64Array::from(relay_timestamps);

        let p2p_timestamps = rows.iter().map(|row| row.p2p_timestamp).collect::<Vec<_>>();
        let p2p_timestamp_array = UInt64Array::from(p2p_timestamps);

        let proposer_fee_recipients = rows
            .iter()
            .map(|row| row.proposer_fee_recipient.map(|rec| rec.0 .0.to_vec()))
            .collect::<Vec<_>>();
        let proposer_fee_recipient_array = BinaryArray::from_opt_vec(
            proposer_fee_recipients
                .iter()
                .map(|vals| vals.as_ref().map(|v| v.as_bytes()))
                .collect::<Vec<_>>(),
        );

        let proposer_mev_rewards = rows
            .iter()
            .map(|row| {
                row.proposer_mev_reward
                    .map(|reward| reward.to_be_bytes().to_vec())
            })
            .collect::<Vec<_>>();

        let proposer_mev_reward_array = BinaryArray::from_opt_vec(
            proposer_mev_rewards
                .iter()
                .map(|vals| vals.as_ref().map(|v| v.as_bytes()))
                .collect::<Vec<_>>(),
        );

        let mempool_flow: Vec<Vec<u8>> = rows
            .into_iter()
            .map(|row| {
                let mut tx_hashes = Vec::new();
                row.mempool_flow
                    .into_iter()
                    .for_each(|tx| tx_hashes.extend(tx.to_vec()));
                tx_hashes
            })
            .collect::<Vec<_>>();

        let mempool_flow_array = BinaryArray::from_iter_values(mempool_flow.into_iter());

        RecordBatch::try_new(
            Arc::new(metadata_schema()),
            vec![
                Arc::new(block_number_array),
                Arc::new(block_hash_array),
                Arc::new(block_timestamp_array),
                Arc::new(relay_timestamp_array),
                Arc::new(p2p_timestamp_array),
                Arc::new(proposer_fee_recipient_array),
                Arc::new(proposer_mev_reward_array),
                Arc::new(mempool_flow_array),
            ],
        )
        .unwrap()
    }
}

pub fn metadata_paquet_file() -> String {
    format!(
        "{}/{}",
        env::var("BRONTES_LIBMDBX_BENCHES_PATH").expect("No BRONTES_LIBMDBX_BENCHES_PATH in .env"),
        METADATA_PARQUET_FILE
    )
}

pub fn metadata_libmdbx_dir() -> String {
    format!(
        "{}/{}",
        env::var("BRONTES_LIBMDBX_BENCHES_PATH").expect("No BRONTES_LIBMDBX_BENCHES_PATH in .env"),
        METADATA_LIBMDBX_DIR
    )
}
