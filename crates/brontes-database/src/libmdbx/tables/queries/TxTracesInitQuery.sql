WITH
    meta AS (
        SELECT 
            tx_hash,
            arrayZip(
                trace_meta.trace_idx,
                trace_meta.msg_sender,
                trace_meta.error,
                trace_meta.subtraces,
                trace_meta.trace_address
            ) AS data
        FROM brontes.tx_traces
    ),
    decoded_data AS (
        SELECT 
            tx_hash,
            arrayZip(
                trace_decoded_data.trace_idx,
                trace_decoded_data.function_name,
                trace_decoded_data.call_data,
                trace_decoded_data.return_data
            ) AS data
        FROM brontes.tx_traces
    ),
    logs AS (
        SELECT 
            tx_hash,
            arrayZip(
                trace_logs.trace_idx,
                trace_logs.log_idx,
                trace_logs.address,
                trace_logs.topics,
                trace_logs.data
            ) AS data
        FROM brontes.tx_traces
    ),
    logs AS (
        SELECT 
            tx_hash,
            arrayZip(
                trace_logs.trace_idx,
                trace_logs.log_idx,
                trace_logs.address,
                trace_logs.topics,
                trace_logs.data
            ) AS data
        FROM brontes.tx_traces
    ),
    actions AS (
        SELECT
            tx_hash,
        arrayZip(
            trace_create_actions.trace_idx,
            trace_create_actions.from,
            trace_create_actions.gas,
            trace_create_actions.init,
            trace_create_actions.value
        ) AS create,
        arrayZip(
            trace_call_actions.trace_idx,
            trace_call_actions.from,
            trace_call_actions.call_type,
            trace_call_actions.gas,
            trace_call_actions.input,
            trace_call_actions.to,
            trace_call_actions.value
        ) AS call,
        arrayZip(
            trace_self_destruct_actions.trace_idx,
            trace_self_destruct_actions.address,
            trace_self_destruct_actions.balance,
            trace_self_destruct_actions.refund_address
        ) AS self_destr,
        arrayZip(
            trace_reward_actions.trace_idx,
            trace_reward_actions.author,
            trace_reward_actions.reward_type,
            trace_reward_actions.value
        ) AS reward
        FROM brontes.tx_traces
    ),
    outputs AS (
        SELECT
            tx_hash,
        arrayZip(
            trace_call_outputs.trace_idx,
            trace_call_outputs.gas_used,
            trace_call_outputs.output
        ) AS call,
        arrayZip(
            trace_create_outputs.trace_idx,
            trace_create_outputs.address,
            trace_create_outputs.code,
            trace_create_outputs.gas_used
        ) AS create
        FROM brontes.tx_traces
    ),
    block_traces AS (
        SELECT
            tx_traces.block_number AS block_number,
            (
                m.data, 
                d.data, 
                l.data, 
                a.create,
                a.call,
                a.self_destr,
                a.reward,
                o.call,
                o.create
            ) AS trace,
            tx_traces.tx_hash AS tx_hash,
            tx_traces.gas_used AS gas_used,
            tx_traces.effective_price AS effective_price,
            tx_traces.tx_index AS tx_index,
            tx_traces.is_success AS is_success
        FROM brontes.tx_traces AS tx_traces
        INNER JOIN meta AS m ON m.tx_hash = tx_traces.tx_hash
        INNER JOIN decoded_data AS d ON d.tx_hash = tx_traces.tx_hash
        INNER JOIN logs AS l ON l.tx_hash = tx_traces.tx_hash
        INNER JOIN actions AS a ON a.tx_hash = tx_traces.tx_hash
        INNER JOIN outputs AS o ON o.tx_hash = tx_traces.tx_hash
    )
SELECT 
    block_number,
    groupArray((block_number, trace, tx_hash, gas_used, effective_price, tx_index, is_success)) 
FROM block_traces
GROUP BY block_number
