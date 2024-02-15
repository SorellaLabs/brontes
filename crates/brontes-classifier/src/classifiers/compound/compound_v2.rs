use alloy_primitives::{hex, Address};
use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{normalized_actions::NormalizedLiquidation, utils::ToScaledRational};

action_impl!(
    Protocol::CompoundV2,
    crate::CompoundV2CToken::liquidateBorrowCall,
    Liquidation,
    [..LiquidateBorrow],
    call_data: true,
    logs: true,
    |trace_index,
    _from_address: Address,
    target_address: Address,
    msg_sender: Address,
    call_data: liquidateBorrowCall,
    log_data: CompoundV2liquidateBorrowCallLogs,
    db_tx: &DB | {
        let tokens = db_tx.get_protocol_tokens(target_address).ok()??;
        let debt_asset = tokens.token0;
        let logs = log_data.LiquidateBorrow_field;
        let debt_info = db_tx.try_get_token_info(debt_asset).ok()??;
        let collateral_info = db_tx.try_get_token_info(call_data.cTokenCollateral).ok()??;

        let covered_debt = logs.repayAmount.to_scaled_rational(debt_info.decimals);
        let liquidated_collateral = logs.seizeTokens.to_scaled_rational(collateral_info.decimals);

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
            liquidated_collateral: liquidated_collateral,
        })
    }
);

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy_primitives::{hex, Address, B256, U256};
    use brontes_classifier::test_utils::ClassifierTestUtils;
    use brontes_types::{
        db::token_info::TokenInfoWithAddress, normalized_actions::Actions, Node,
        Protocol::CompoundV2, ToScaledRational, TreeSearchArgs,
    };

    use super::*;

    #[brontes_macros::test]
    async fn test_compound_v2_liquidation() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let compound_v2_liquidation =
            B256::from(hex!("3a3ba6b0a6b69a8e316e1c20f97b9ce2de790b2f3bf90aaef5b29b06aafa5fda"));

        let eq_action = Actions::Liquidation(NormalizedLiquidation {
            protocol:              Protocol::CompoundV2,
            liquidated_collateral: Rational::from_signeds(48779241727, 100000000),
            covered_debt:          Rational::from_signeds(6140057900131_i64, 1000000),
            debtor:                Address::from(hex!("De74395831F3Ba9EdC7cBEE1fcB441cf24c0AF4d")),
            debt_asset:            classifier_utils
                .get_token_info(Address::from(hex!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"))),
            collateral_asset:      classifier_utils
                .get_token_info(Address::from(hex!("70e36f6BF80a52b3B46b3aF8e106CC0ed743E8e4"))),
            liquidator:            Address::from(hex!("D911560979B78821D7b045C79E36E9CbfC2F6C6F")),
            pool:                  Address::from(hex!(
                "0x39AA39c021dfbaE8faC545936693aC917d5E7563"
            )),
            trace_index:           6,
        });

        let search_fn = |node: &Node<Actions>| TreeSearchArgs {
            collect_current_node:  node.data.is_liquidation(),
            child_node_to_collect: node.subactions.iter().any(|action| action.is_liquidation()),
        };

        classifier_utils
            .contains_action(compound_v2_liquidation, 0, eq_action, search_fn)
            .await
            .unwrap();
    }
}
