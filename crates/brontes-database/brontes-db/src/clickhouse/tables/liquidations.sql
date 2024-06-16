CREATE TABLE mev.liquidations ON CLUSTER eth_cluster0
(
    `liquidation_tx_hash` String,
    `liquidation_swaps` Nested(
        `trace_idx` UInt64,
        `from` String,
        `recipient` String,
        `pool` String,
        `token_in` Tuple(String, String),
        `token_out` Tuple(String, String),
        `amount_in` Tuple(UInt256, UInt256),
        `amount_out` Tuple(UInt256, UInt256)
    ),
    `liquidations` Nested(
        `trace_idx` UInt64,
        `pool` String,
        `liquidator` String,
        `debtor` String,
        `collateral_asset` Tuple(String, String),
        `debt_asset` Tuple(String, String),
        `covered_debt` Tuple(UInt256, UInt256),
        `liquidated_collateral` Tuple(UInt256, UInt256)
      ),
    `gas_details` Tuple(
        `coinbase_transfer` Nullable(UInt128), 
        `priority_fee` UInt128,
        `gas_used` UInt128,
        `effective_gas_price` UInt128
    ),
    `last_updated` UInt64 DEFAULT now()
) 
ENGINE = ReplicatedReplacingMergeTree('/clickhouse/eth_cluster0/tables/all/mev/liquidations', '{replica}', `last_updated`)
PRIMARY KEY (`liquidation_tx_hash`)
ORDER BY (`liquidation_tx_hash` )
