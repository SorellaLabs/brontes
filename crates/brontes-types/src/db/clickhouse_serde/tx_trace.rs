use itertools::Itertools;
use reth_rpc_types::trace::parity::{Action, TraceOutput};

use crate::structured_trace::TxTrace;

#[derive(Debug, Default)]
pub struct ClickhouseDecodedCallData {
    pub trace_idx: Vec<u64>,
    pub function_name: Vec<String>,
    pub call_data: Vec<Vec<(String, String, String)>>,
    pub return_data: Vec<Vec<(String, String, String)>>,
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

        println!("THIS trace_idx: {:?}\n", this.trace_idx);
        println!("THIS function_name: {:?}\n", this.function_name);
        println!("THIS call_data: {:?}\n", this.call_data);
        println!("THIS return_data: {:?}\n", this.return_data);
        this
    }
}

#[derive(Debug, Default)]
pub struct ClickhouseLogs {
    pub trace_idx: Vec<u64>,
    pub log_idx: Vec<u64>,
    pub address: Vec<String>,
    pub topics: Vec<Vec<String>>,
    pub data: Vec<String>,
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
    pub from: Vec<String>,
    pub gas: Vec<u64>,
    pub init: Vec<String>,
    pub value: Vec<[u8; 32]>,
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
                    this.from.push(format!("{:?}", c.from));
                    this.gas.push(c.gas.to::<u64>());
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
    pub from: Vec<String>,
    pub call_type: Vec<String>,
    pub gas: Vec<u64>,
    pub input: Vec<String>,
    pub to: Vec<String>,
    pub value: Vec<[u8; 32]>,
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
                    this.from.push(format!("{:?}", c.from));
                    this.call_type.push(format!("{:?}", c.call_type));
                    this.gas.push(c.gas.to::<u64>());
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
    pub address: Vec<String>,
    pub balance: Vec<[u8; 32]>,
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
    pub author: Vec<String>,
    pub value: Vec<[u8; 32]>,
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
    pub gas_used: Vec<u64>,
    pub output: Vec<String>,
}

impl<'a> From<&'a TxTrace> for ClickhouseCallOutput {
    fn from(value: &'a TxTrace) -> Self {
        let mut this = Self::default();

        value
            .trace
            .iter()
            .filter_map(|trace| {
                trace
                    .trace
                    .result
                    .as_ref()
                    .map(|res| match res {
                        TraceOutput::Call(c) => {
                            Some((c.gas_used.to::<u64>(), format!("{:?}", c.output)))
                        }
                        _ => None,
                    })
                    .flatten()
            })
            .for_each(|(g, o)| {
                this.gas_used.push(g);
                this.output.push(o);
            });

        this
    }
}

#[derive(Debug, Default)]
pub struct ClickhouseCreateOutput {
    pub address: Vec<String>,
    pub code: Vec<String>,
    pub gas_used: Vec<u64>,
}

impl<'a> From<&'a TxTrace> for ClickhouseCreateOutput {
    fn from(value: &'a TxTrace) -> Self {
        let mut this = Self::default();

        value
            .trace
            .iter()
            .filter_map(|trace| {
                trace
                    .trace
                    .result
                    .as_ref()
                    .map(|res| match res {
                        TraceOutput::Create(c) => Some((
                            format!("{:?}", c.address),
                            format!("{:?}", c.code),
                            c.gas_used.to::<u64>(),
                        )),
                        _ => None,
                    })
                    .flatten()
            })
            .for_each(|(a, c, g)| {
                this.address.push(a);
                this.code.push(c);
                this.gas_used.push(g);
            });

        this
    }
}
