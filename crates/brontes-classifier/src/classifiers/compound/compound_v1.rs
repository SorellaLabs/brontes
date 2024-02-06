use alloy_primitives::{Address, b256, U256};
use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::normalized_actions:: NormalizedLiquidation;
use malachite::{num::basic::traits::Zero, Rational};

action_impl!(
    Protocol::CompoundV2,
    crate::CompoundV2CEther::liquidateBorrowCall,
    Liquidation,
    [LiquidationEvent],
    call_data: true,
    |trace_index,
    _from_address: Address,
    target_address: Address,
    msg_sender: Address,
    call_data: liquidateBorrowCall,
    db_tx: &DB | {
        let debt_asset: Address = Address::from_word(b256!("0000000000000000000000004Ddc2D193948926D02f9B1fE9e1daa0718270ED5"));
        let debt_info = db_tx.try_get_token_info(debt_asset).ok()??;
        let collateral_info = db_tx.try_get_token_info(call_data.cTokenCollateral).ok()??;

        let covered_debt = msg_value.to_scaled_rational(debt_info.decimals);

        return Some(NormalizedLiquidation {
            protocol: Protocol::CompoundV2,
            trace_index,
            pool: target_address,
            liquidator: msg_sender,
            debtor: call_data.borrower,
            collateral_asset: collateral_info,
            debt_asset: debt_info,
            covered_debt: covered_debt,
            // filled in later
            liquidated_collateral: Rational::ZERO,
        })
    }
);

