use alloy_primitives::{Address, Bytes};
use alloy_sol_types::SolCall;
use brontes_types::{
    db::traits::LibmdbxReader, normalized_actions::NormalizedTransfer, ToScaledRational,
};
use malachite::{num::basic::traits::Zero, Rational};

alloy_sol_macro::sol!(
    function transfer(address, uint) returns(bool);
    function transferFrom(address, address, uint) returns(bool);
);

pub fn try_decode_transfer<DB: LibmdbxReader>(
    idx: u64,
    calldata: Bytes,
    from: Address,
    token: Address,
    db: &DB,
) -> eyre::Result<NormalizedTransfer> {
    let (from_addr, to_addr, amount) = transferCall::abi_decode(&calldata, false)?
        .map(|t| Some((from, t._0, t._1)))
        .unwrap_or_else(|_| {
            transferFromCall::abi_decode(&calldata, false).map(|t| (t._0, t._1, t._2))
        })?;
    let token_info = db.try_fetch_token_info(token).ok()??;

    Ok(NormalizedTransfer {
        amount:      amount.to_scaled_rational(token_info.decimals),
        token:       token_info,
        to:          to_addr,
        from:        from_addr,
        trace_index: idx,
        fee:         Rational::ZERO,
    })
}
