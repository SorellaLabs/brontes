







CREATE TABLE IF NOT EXISTS brontes.tx_traces ON CLUSTER eth_cluster0 (
    `tx_hash` String,
    `gas_used` UInt128,
    `effective_price` UInt128,
    `tx_index` UInt64,
    `is_success` Bool,
    `trace_info` Nested (
        `trace_idx` UInt64,
        `msg_sender` String
    ),
    `trace_data` Nested (
        `trace_idx` UInt64,
        `function_name` String,
        `call_data` Array(Tuple(String, String, String)),
        `return_data` Array(Tuple(String, String, String))
    ),
    `trace_logs` Nested (
        `trace_idx` UInt64,
        `logs` Array(Tuple(String, Array(String), String))
    ),
    `tx_trace` Nested(
        `trace_idx` UInt64,
        `error` Nullable(String),
        `subtraces` UInt64,
        `trace_address` Array(UInt64),
        `create_action` Nullable(Tuple(String, UInt64, String, UInt256)),
        `call_action` Nullable(Tuple(String, String, UInt64, String, String, UInt256)),
        `self_destruct_action` Nullable(Tuple(String, String, UInt256)),
        `reward_action` Nullable(Tuple(String, UInt256, String)),
        `call_output` Nullable(Tuple(UInt64, String)),
        `create_output` Nullable(Tuple(String, String, UInt64))
    )
)
ENGINE = ReplicatedReplacingMergeTree('/clickhouse/eth_cluster0/tables/brontes/all/tx_traces', '{replica}')
ORDER BY `tx_hash`