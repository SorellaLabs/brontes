use alloy_rpc_types_trace::parity::{Action, TraceOutput};
use itertools::Itertools;

use crate::structured_trace::TxTrace;

#[derive(Debug, Default)]
pub struct ClickhouseDecodedCallData {
    pub trace_idx:     Vec<u64>,
    pub function_name: Vec<String>,
    pub call_data:     Vec<Vec<(String, String, String)>>,
    pub return_data:   Vec<Vec<(String, String, String)>>,
}
impl<'a> From<&'a TxTrace> for ClickhouseDecodedCallData {
    fn from(value: &'a TxTrace) -> Self {
        let mut this = Self::default();
        value
            .trace
            .iter()
            .filter_map(|trace| {
                trace.decoded_data.as_ref().map(|data| {
                    (
                        trace.trace_idx,
                        (
                            data.function_name.clone(),
                            data.call_data
                                .iter()
                                .map(|d| {
                                    (d.field_name.clone(), d.field_type.clone(), d.value.clone())
                                })
                                .collect_vec(),
                            data.return_data
                                .iter()
                                .map(|d| {
                                    (d.field_name.clone(), d.field_type.clone(), d.value.clone())
                                })
                                .collect_vec(),
                        ),
                    )
                })
            })
            .for_each(|(trace_idx, (f, c, r))| {
                this.trace_idx.push(trace_idx);
                this.function_name.push(f);
                this.call_data.push(c);
                this.return_data.push(r);
            });

        this
    }
}

#[derive(Debug, Default)]
pub struct ClickhouseLogs {
    pub trace_idx: Vec<u64>,
    pub log_idx:   Vec<u64>,
    pub address:   Vec<String>,
    pub topics:    Vec<Vec<String>>,
    pub data:      Vec<String>,
}

impl<'a> From<&'a TxTrace> for ClickhouseLogs {
    fn from(value: &'a TxTrace) -> Self {
        let mut this = Self::default();
        value
            .trace
            .iter()
            .flat_map(|trace| {
                trace
                    .logs
                    .iter()
                    .enumerate()
                    .map(|(log_idx, log)| {
                        (
                            trace.trace_idx,
                            log_idx as u64,
                            format!("{:?}", log.address),
                            log.topics()
                                .iter()
                                .map(|topic| format!("{:?}", topic))
                                .collect_vec(),
                            format!("{:?}", log.data.data),
                        )
                    })
                    .collect_vec()
            })
            .for_each(|(t_idx, l, a, t, d)| {
                this.trace_idx.push(t_idx);
                this.log_idx.push(l);
                this.address.push(a);
                this.topics.push(t);
                this.data.push(d);
            });

        this
    }
}

#[derive(Debug, Default)]
pub struct ClickhouseCreateAction {
    pub trace_idx: Vec<u64>,
    pub from:      Vec<String>,
    pub gas:       Vec<u64>,
    pub init:      Vec<String>,
    pub value:     Vec<[u8; 32]>,
}

impl<'a> From<&'a TxTrace> for ClickhouseCreateAction {
    fn from(value: &'a TxTrace) -> Self {
        let mut this = Self::default();

        value
            .trace
            .iter()
            .filter(|trace| trace.trace.action.is_create())
            .for_each(|trace| match &trace.trace.action {
                Action::Create(c) => {
                    this.trace_idx.push(trace.trace_idx);
                    this.from.push(format!("{:?}", c.from));
                    this.gas.push(c.gas);
                    this.init.push(format!("{:?}", c.init));
                    this.value.push(c.value.to_le_bytes() as [u8; 32]);
                }
                _ => unreachable!(),
            });

        this
    }
}

#[derive(Debug, Default)]
pub struct ClickhouseCallAction {
    pub trace_idx: Vec<u64>,
    pub from:      Vec<String>,
    pub call_type: Vec<String>,
    pub gas:       Vec<u64>,
    pub input:     Vec<String>,
    pub to:        Vec<String>,
    pub value:     Vec<[u8; 32]>,
}

impl<'a> From<&'a TxTrace> for ClickhouseCallAction {
    fn from(value: &'a TxTrace) -> Self {
        let mut this = Self::default();

        value
            .trace
            .iter()
            .filter(|trace| trace.trace.action.is_call())
            .for_each(|trace| match &trace.trace.action {
                Action::Call(c) => {
                    this.trace_idx.push(trace.trace_idx);
                    this.from.push(format!("{:?}", c.from));
                    this.call_type.push(format!("{:?}", c.call_type));
                    this.gas.push(c.gas);
                    this.input.push(format!("{:?}", c.input));
                    this.to.push(format!("{:?}", c.to));
                    this.value.push(c.value.to_le_bytes() as [u8; 32]);
                }
                _ => unreachable!(),
            });

        this
    }
}

#[derive(Debug, Default)]
pub struct ClickhouseSelfDestructAction {
    pub trace_idx:      Vec<u64>,
    pub address:        Vec<String>,
    pub balance:        Vec<[u8; 32]>,
    pub refund_address: Vec<String>,
}

impl<'a> From<&'a TxTrace> for ClickhouseSelfDestructAction {
    fn from(value: &'a TxTrace) -> Self {
        let mut this = Self::default();

        value
            .trace
            .iter()
            .filter(|trace| trace.trace.action.is_selfdestruct())
            .for_each(|trace| match &trace.trace.action {
                Action::Selfdestruct(c) => {
                    this.trace_idx.push(trace.trace_idx);
                    this.address.push(format!("{:?}", c.address));
                    this.refund_address.push(format!("{:?}", c.refund_address));
                    this.balance.push(c.balance.to_le_bytes() as [u8; 32]);
                }
                _ => unreachable!(),
            });

        this
    }
}

#[derive(Debug, Default)]
pub struct ClickhouseRewardAction {
    pub trace_idx:   Vec<u64>,
    pub author:      Vec<String>,
    pub value:       Vec<[u8; 32]>,
    pub reward_type: Vec<String>,
}

impl<'a> From<&'a TxTrace> for ClickhouseRewardAction {
    fn from(value: &'a TxTrace) -> Self {
        let mut this = Self::default();

        value
            .trace
            .iter()
            .filter(|trace| trace.trace.action.is_reward())
            .for_each(|trace| match &trace.trace.action {
                Action::Reward(c) => {
                    this.trace_idx.push(trace.trace_idx);
                    this.author.push(format!("{:?}", c.author));
                    this.reward_type.push(format!("{:?}", c.reward_type));
                    this.value.push(c.value.to_le_bytes() as [u8; 32]);
                }
                _ => unreachable!(),
            });

        this
    }
}

#[derive(Debug, Default)]
pub struct ClickhouseCallOutput {
    pub trace_idx: Vec<u64>,
    pub gas_used:  Vec<u64>,
    pub output:    Vec<String>,
}

impl<'a> From<&'a TxTrace> for ClickhouseCallOutput {
    fn from(value: &'a TxTrace) -> Self {
        let mut this = Self::default();

        value
            .trace
            .iter()
            .filter_map(|trace| {
                trace.trace.result.as_ref().and_then(|res| match res {
                    TraceOutput::Call(c) => {
                        Some((trace.trace_idx, c.gas_used, format!("{:?}", c.output)))
                    }
                    _ => None,
                })
            })
            .for_each(|(i, g, o)| {
                this.trace_idx.push(i);
                this.gas_used.push(g);
                this.output.push(o);
            });

        this
    }
}

#[derive(Debug, Default)]
pub struct ClickhouseCreateOutput {
    pub trace_idx: Vec<u64>,
    pub address:   Vec<String>,
    pub code:      Vec<String>,
    pub gas_used:  Vec<u64>,
}

impl<'a> From<&'a TxTrace> for ClickhouseCreateOutput {
    fn from(value: &'a TxTrace) -> Self {
        let mut this = Self::default();

        value
            .trace
            .iter()
            .filter_map(|trace| {
                trace.trace.result.as_ref().and_then(|res| match res {
                    TraceOutput::Create(c) => Some((
                        trace.trace_idx,
                        format!("{:?}", c.address),
                        format!("{:?}", c.code),
                        c.gas_used,
                    )),
                    _ => None,
                })
            })
            .for_each(|(i, a, c, g)| {
                this.trace_idx.push(i);
                this.address.push(a);
                this.code.push(c);
                this.gas_used.push(g);
            });

        this
    }
}

pub mod tx_traces_inner {
    use std::str::FromStr;

    use alloy_primitives::{Address, Bytes, Log, LogData, TxHash, U256, U64};
    use alloy_rpc_types_trace::parity::{
        Action, CallAction, CallOutput, CallType, CreateAction, CreateOutput, CreationMethod,
        RewardAction, RewardType, SelfdestructAction, TraceOutput, TransactionTrace,
    };
    use itertools::Itertools;
    use serde::de::{Deserialize, Deserializer};

    use crate::{
        db::traces::TxTracesInner,
        structured_trace::{DecodedCallData, DecodedParams, TransactionTraceWithLogs, TxTrace},
        FastHashMap,
    };

    type TxTraceClickhouseTuple = (
        u64,
        (
            Vec<(u64, String, Option<String>, u64, Vec<u64>)>, // meta
            Vec<(u64, String, Vec<(String, String, String)>, Vec<(String, String, String)>)>,
            Vec<(u64, u64, String, Vec<String>, String)>, // logs
            Vec<(u64, String, u64, String, [u8; 32])>,    // create action
            Vec<(u64, String, String, u64, String, String, [u8; 32])>, // call action
            Vec<(u64, String, [u8; 32], String)>,         // self destruct action
            Vec<(u64, String, String, [u8; 32])>,         // reward action
            Vec<(u64, u64, String)>,                      // call output
            Vec<(u64, String, String, u64)>,              // create output
        ),
        String,
        u128,
        u128,
        u64,
        bool,
    );

    fn des_tx_trace(value: TxTraceClickhouseTuple) -> TxTrace {
        let mut tx_trace = TxTrace::default();

        let default_trace = TransactionTraceWithLogs {
            trace:        TransactionTrace {
                action:        Action::Selfdestruct(SelfdestructAction {
                    address:        Default::default(),
                    balance:        Default::default(),
                    refund_address: Default::default(),
                }),
                error:         None,
                result:        None,
                subtraces:     0,
                trace_address: Vec::new(),
            },
            logs:         Vec::new(),
            msg_sender:   Default::default(),
            trace_idx:    Default::default(),
            decoded_data: None,
        };

        let (
            block_num,
            (
                meta,
                decoded_data,
                logs,
                create_action,
                call_action,
                self_destruct_action,
                reward_action,
                call_output,
                create_output,
            ),
            tx_hash,
            gas_used,
            effective_price,
            tx_index,
            is_success,
        ) = value;

        tx_trace.block_number = block_num;
        tx_trace.tx_hash = TxHash::from_str(&tx_hash).unwrap();
        tx_trace.gas_used = gas_used;
        tx_trace.effective_price = effective_price;
        tx_trace.tx_index = tx_index;
        tx_trace.is_success = is_success;

        let mut map = FastHashMap::default();

        // meta
        meta.into_iter()
            .for_each(|(trace_idx, msg_sender, error, subtraces, trace_address)| {
                let entry = map.entry(trace_idx).or_insert(default_trace.clone());

                entry.msg_sender = Address::from_str(&msg_sender).unwrap();
                entry.trace.error = error;
                entry.trace.subtraces = subtraces as usize;
                entry.trace.trace_address = trace_address.into_iter().map(|v| v as usize).collect();
                entry.trace_idx = trace_idx;
            });

        // decoded_data
        decoded_data
            .into_iter()
            .for_each(|(trace_idx, function_name, call_data, return_data)| {
                let entry = map.entry(trace_idx).or_insert(default_trace.clone());

                let decoded_data = DecodedCallData {
                    function_name,
                    call_data: call_data
                        .into_iter()
                        .map(|(field_name, field_type, value)| DecodedParams {
                            field_name,
                            field_type,
                            value,
                        })
                        .collect_vec(),
                    return_data: return_data
                        .into_iter()
                        .map(|(field_name, field_type, value)| DecodedParams {
                            field_name,
                            field_type,
                            value,
                        })
                        .collect_vec(),
                };

                entry.decoded_data = Some(decoded_data);
            });

        // logs
        let mut log_map = FastHashMap::default();
        logs.into_iter()
            .for_each(|(trace_idx, log_idx, address, topics, data)| {
                let log_entry = log_map.entry(trace_idx).or_insert(FastHashMap::default());

                log_entry.insert(
                    log_idx,
                    Log {
                        address: Address::from_str(&address).unwrap(),
                        data:    LogData::new_unchecked(
                            topics
                                .into_iter()
                                .map(|t| TxHash::from_str(&t).unwrap())
                                .collect_vec(),
                            Bytes::from_str(&data).unwrap(),
                        ),
                    },
                );
            });
        log_map.into_iter().for_each(|(trace_idx, log_map)| {
            let max_idx = log_map.len();

            let trace_entry = map.entry(trace_idx).or_insert(default_trace.clone());

            (0..max_idx).for_each(|i| {
                trace_entry
                    .logs
                    .push(log_map.get(&(i as u64)).cloned().unwrap())
            })
        });

        // create_action
        create_action
            .into_iter()
            .for_each(|(trace_idx, from, gas, init, value)| {
                let entry = map.entry(trace_idx).or_insert(default_trace.clone());

                let create = CreateAction {
                    from: Address::from_str(&from).unwrap(),
                    gas,
                    init: Bytes::from_str(&init).unwrap(),
                    value: U256::from_le_bytes(value),
                    creation_method: CreationMethod::default(),
                };

                entry.trace.action = Action::Create(create);
            });

        // call_action
        call_action
            .into_iter()
            .for_each(|(trace_idx, from, call_type, gas, input, to, value)| {
                let entry = map.entry(trace_idx).or_insert(default_trace.clone());

                let call_type = if call_type.as_str() == "Call" {
                    CallType::Call
                } else if call_type.as_str() == "CallCode" {
                    CallType::CallCode
                } else if call_type.as_str() == "DelegateCall" {
                    CallType::DelegateCall
                } else if call_type.as_str() == "StaticCall" {
                    CallType::StaticCall
                } else {
                    CallType::None
                };

                let call = CallAction {
                    from: Address::from_str(&from).unwrap(),
                    gas,
                    value: U256::from_le_bytes(value),
                    call_type,
                    input: Bytes::from_str(&input).unwrap(),
                    to: Address::from_str(&to).unwrap(),
                };

                entry.trace.action = Action::Call(call);
            });

        // self_destruct_action
        self_destruct_action.into_iter().for_each(
            |(trace_idx, address, balance, refund_address)| {
                let entry = map.entry(trace_idx).or_insert(default_trace.clone());

                let self_destruct = SelfdestructAction {
                    address:        Address::from_str(&address).unwrap(),
                    balance:        U256::from_le_bytes(balance),
                    refund_address: Address::from_str(&refund_address).unwrap(),
                };

                entry.trace.action = Action::Selfdestruct(self_destruct);
            },
        );

        // reward_action
        reward_action
            .into_iter()
            .for_each(|(trace_idx, author, reward_type, value)| {
                let entry = map.entry(trace_idx).or_insert(default_trace.clone());

                let reward_type = if reward_type.as_str() == "Block" {
                    RewardType::Block
                } else if reward_type.as_str() == "Uncle" {
                    RewardType::Uncle
                } else {
                    unreachable!(
                        "reward type must be either 'Block' or 'Uncle' - have: {}",
                        reward_type
                    )
                };

                let reward = RewardAction {
                    author: Address::from_str(&author).unwrap(),
                    reward_type,
                    value: U256::from_le_bytes(value),
                };

                entry.trace.action = Action::Reward(reward);
            });

        // call_output
        call_output
            .into_iter()
            .for_each(|(trace_idx, gas_used, output)| {
                let entry = map.entry(trace_idx).or_insert(default_trace.clone());

                let call = CallOutput { gas_used, output: Bytes::from_str(&output).unwrap() };

                entry.trace.result = Some(TraceOutput::Call(call))
            });

        // create_output
        create_output
            .into_iter()
            .for_each(|(trace_idx, address, code, gas_used)| {
                let entry = map.entry(trace_idx).or_insert(default_trace.clone());

                let create = CreateOutput {
                    gas_used,
                    address: Address::from_str(&address).unwrap(),
                    code: Bytes::from_str(&code).unwrap(),
                };

                entry.trace.result = Some(TraceOutput::Create(create))
            });

        let mut tx_traces_with_logs = map.into_iter().collect_vec();
        tx_traces_with_logs.sort_by_key(|(idx, _)| *idx);

        tx_trace.trace = tx_traces_with_logs
            .into_iter()
            .map(|(_, trace)| trace)
            .collect_vec();

        tx_trace
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<TxTracesInner, D::Error>
    where
        D: Deserializer<'de>,
    {
        let values: Vec<TxTraceClickhouseTuple> = Deserialize::deserialize(deserializer)?;

        let converted = values.into_iter().map(des_tx_trace).collect_vec();

        if converted.is_empty() {
            Ok(TxTracesInner { traces: None })
        } else {
            Ok(TxTracesInner { traces: Some(converted) })
        }
    }
}
