use alloy_primitives::{Address, FixedBytes, Log, B256, U256};
use alloy_rpc_types_trace::parity::Action;
use hex_literal::hex;

pub(crate) fn get_coinbase_transfer(builder: Address, action: &Action) -> Option<u128> {
    match action {
        Action::Call(action) => {
            if action.to == builder && !action.value.is_zero() {
                return Some(action.value.to())
            }
            None
        }
        _ => None,
    }
}

const TRANSFER_TOPIC: B256 =
    FixedBytes(hex!("ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"));

pub(crate) fn decode_transfer(log: &Log) -> Option<(Address, Address, Address, U256)> {
    if log.topics().len() != 3 {
        return None
    }

    if log.topics().first() == Some(&TRANSFER_TOPIC) {
        let from = Address::from_slice(&log.topics()[1][12..]);
        let to = Address::from_slice(&log.topics()[2][12..]);
        let data = U256::try_from_be_slice(&log.data.data[..])?;
        return Some((log.address, from, to, data))
    }

    None
}
