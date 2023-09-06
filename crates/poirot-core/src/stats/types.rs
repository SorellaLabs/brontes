use crate::errors::TraceParseErrorKind;
use colored::Colorize;
use reth_primitives::H256;
use tracing::info;

#[derive(Clone, Debug)]
pub struct BlockStats {
    pub(crate) block_num: u64,
    pub(crate) txs: Vec<TransactionStats>,
    pub(crate) err: Option<TraceParseErrorKind>,
}

impl BlockStats {
    pub(crate) fn new(block_num: u64, err: Option<TraceParseErrorKind>) -> Self {
        Self { block_num, txs: Vec::new(), err }
    }

    pub(crate) fn trace(&self) {
        let message = format!(
            "Successfuly Parsed Block {}",
            format!("{}", self.block_num).bright_blue().bold()
        );
        info!(message = message);
    }
}

#[derive(Clone, Debug)]
pub struct TransactionStats {
    pub(crate) block_num: u64,
    pub(crate) tx_hash: H256,
    pub(crate) tx_idx: u16,
    pub(crate) traces: Vec<TraceStats>,
    pub(crate) err: Option<TraceParseErrorKind>,
}

impl TransactionStats {
    pub(crate) fn new(
        block_num: u64,
        tx_hash: H256,
        tx_idx: u16,
        err: Option<TraceParseErrorKind>,
    ) -> Self {
        Self { block_num, tx_hash, tx_idx, traces: Vec::new(), err }
    }

    pub(crate) fn trace(&self) {
        let tx_hash = format!("{:#x}", self.tx_hash);
        info!("result = \"Successfully Parsed Transaction\", tx_hash = {}\n", tx_hash);
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TraceStats {
    pub(crate) block_num: u64,
    pub(crate) tx_hash: H256,
    pub(crate) tx_idx: u16,
    pub(crate) trace_idx: u16,
    pub(crate) err: Option<TraceParseErrorKind>,
}

impl TraceStats {
    pub(crate) fn new(
        block_num: u64,
        tx_hash: H256,
        tx_idx: u16,
        trace_idx: u16,
        err: Option<TraceParseErrorKind>,
    ) -> Self {
        Self { block_num, tx_hash, tx_idx, trace_idx, err }
    }

    pub(crate) fn trace(&self, total_len: usize) {
        let tx_hash = format!("{:#x}", self.tx_hash);
        let message = format!(
            "{}",
            format!("Starting Transaction Trace {} / {}", self.trace_idx + 1, &total_len)
                .bright_blue()
                .bold()
        );
        info!(message = message, tx_hash = tx_hash);
    }
}
