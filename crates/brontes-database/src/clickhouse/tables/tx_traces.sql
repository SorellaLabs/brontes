CREATE TABLE brontes.tx_traces ON CLUSTER eth_cluster0
(
    `block_number` UInt64,
    `tx_hash` String,
    `gas_used` UInt128,
    `effective_price` UInt128,
    `tx_index` UInt64,
    `is_success` Bool,
    `trace_meta` Nested (
        `trace_idx` UInt64,
        `msg_sender` String,
        `error` Nullable(String),
        `subtraces` UInt64,
        `trace_address` Array(UInt64)
    ),
    `trace_decoded_data` Nested (
        `trace_idx` UInt64,
        `function_name` String,
        `call_data` Array(Tuple(String, String, String)),
        `return_data` Array(Tuple(String, String, String))
    ),
    `trace_logs` Nested (
        `trace_idx` UInt64,
        `log_idx` UInt64,
        `address` String,
        `topics` Array(String),
        `data` String
    ),
    `trace_create_actions` Nested (
        `trace_idx` UInt64,
        `from` String,
        `gas` UInt64,
        `init` String, 
        `value` UInt256
    ),
    `trace_call_actions` Nested (
        `trace_idx` UInt64,
        `from` String,
        `call_type` String,
        `gas` UInt64,
        `input` String,
        `to` String,
        `value` UInt256
    ),
    `trace_self_destruct_actions` Nested (
        `trace_idx` UInt64,
        `address` String,
        `balance` UInt256,
        `refund_address` String
    ),
    `trace_reward_actions` Nested (
        `trace_idx` UInt64,
        `author` String,
        `reward_type` String,
        `value` UInt256
    ),
    `trace_call_outputs` Nested (
        `trace_idx` UInt64,
        `gas_used` UInt64,
        `output` String
    ),
    `trace_create_outputs` Nested (
        `trace_idx` UInt64,
        `address` String,
        `code` String,
        `gas_used` UInt64
    )
)
ENGINE = ReplicatedReplacingMergeTree('/clickhouse/eth_cluster0/tables/{shard}/brontes/tx_traces', '{replica}')
ORDER BY (`block_number`, `tx_hash`)