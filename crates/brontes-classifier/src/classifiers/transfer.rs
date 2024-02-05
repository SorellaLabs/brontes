use alloy_primitives::{Address, Bytes};
use alloy_sol_types::SolCall;
use brontes_types::{
    db::traits::LibmdbxReader, normalized_actions::NormalizedTransfer, ToScaledRational,
};
use malachite::{num::basic::traits::Zero, Rational};

alloy_sol_macro::sol!(
    function transfer(address, uint) returns(bool);
);

pub fn try_decode_transfer<DB: LibmdbxReader>(
    idx: u64,
    calldata: Bytes,
    from: Address,
    token: Address,
    db: &DB,
) -> Option<NormalizedTransfer> {
    let res = transferCall::abi_decode(&calldata, false).ok()?;
    let token_info = db.try_get_token_info(token).ok()??;

    Some(NormalizedTransfer {
        amount: res._1.to_scaled_rational(token_info.decimals),
        token: token_info,
        to: res._0,
        from,
        trace_index: idx,
        fee: Rational::ZERO,
    })
}
