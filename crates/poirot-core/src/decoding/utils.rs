use crate::errors::TraceParseError;

use super::*;
use alloy_dyn_abi::{DynSolType, ResolveSolType};
use alloy_json_abi::{JsonAbi, StateMutability};
use alloy_sol_types::sol;
use reth_primitives::{H160, H256, U256};
use reth_rpc_types::trace::parity::{
    Action, Action as RethAction, CallAction as RethCallAction, TransactionTrace,
};

sol! {
    interface IDiamondLoupe {
        function facetAddress(bytes4 _functionSelector) external view returns (address facetAddress_);
    }
}
