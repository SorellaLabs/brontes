use alloy_primitives::U256;
use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    normalized_actions::{NormalizedBurn, NormalizedCollect, NormalizedMint, NormalizedSwap},
    structured_trace::CallInfo,
    ToScaledRational,
};

action_impl!(
    Protocol::LFJ,
    crate::LFJPair::swapCall,
    Swap,
    [Swap],
    call_data: true,
    logs: true,
    |
    info: CallInfo,
    call_data: swapCall,
    _logs: LFJSwapCallLogs,
    db_tx: &DB| {
        let swap_field=_logs.swap_field?;
        let amount_in_bytes = swap_field.amountsIn;
        let amount_out_bytes = swap_field.amountsOut;

        let amount_in = U256::from_be_bytes(amount_in_bytes.into());
        let amount_out = U256::from_be_bytes(amount_out_bytes.into());
        let recipient = swap_field.to;
        let details = db_tx.get_protocol_details_sorted(info.target_address)?;
        let [token_0, token_1] = [details.token0, details.token1];

        let t0_info = db_tx.try_fetch_token_info(token_0)?;
        let t1_info = db_tx.try_fetch_token_info(token_1)?;

        let (amount_in, amount_out, token_in, token_out) = if call_data.swapForY {
            (
                amount_in.to_scaled_rational(t0_info.decimals),
                amount_out.to_scaled_rational(t1_info.decimals),
                t0_info,
                t1_info,
            )
        } else {
            (
                amount_in.to_scaled_rational(t1_info.decimals),
                amount_out.to_scaled_rational(t0_info.decimals),
                t1_info,
                t0_info,
            )
        };

        Ok(NormalizedSwap {
            protocol: Protocol::LFJ,
            trace_index: info.trace_idx,
            from: info.from_address,
            pool: info.target_address,
            recipient,
            token_in,
            token_out,
            amount_in,
            amount_out,
            msg_value: info.msg_value
        })
    }
);
action_impl!(
    Protocol::LFJ,
    crate::LFJPair::mintCall,
    Mint,
    [DepositedToBins],
    logs: true,
    call_data: true,
     |
     info: CallInfo,
     call_data: mintCall,
     _logs: LFJMintCallLogs,  db_tx: &DB| {
        let deposited_to_bins_field = _logs.deposited_to_bins_field?;
        let token_deltas = deposited_to_bins_field.amounts;
        let token_0_delta = U256::from_be_bytes(token_deltas[0].into());
        let token_1_delta = U256::from_be_bytes(token_deltas[1].into());

        let details = db_tx.get_protocol_details_sorted(info.target_address)?;
        let [token_0, token_1] = [details.token0, details.token1];

        let t0_info = db_tx.try_fetch_token_info(token_0)?;
        let t1_info = db_tx.try_fetch_token_info(token_1)?;

        let am0 = token_0_delta.to_scaled_rational(t0_info.decimals);
        let am1 = token_1_delta.to_scaled_rational(t1_info.decimals);

        Ok(NormalizedMint {
            protocol: Protocol::LFJ,
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient: call_data.to,
            pool: info.target_address,
            token: vec![t0_info, t1_info],
            amount: vec![am0, am1],
        })
    }
);
action_impl!(
    Protocol::LFJ,
    crate::LFJPair::burnCall,
    Burn,
    [WithdrawnFromBins],
    logs:true,
    |
    info: CallInfo,
    _logs: LFJBurnCallLogs,
    db_tx: &DB| {
        let withdrawn_from_bins_field = _logs.withdrawn_from_bins_field?;
        let token_deltas = withdrawn_from_bins_field.amounts;
        let token_0_delta = U256::from_be_bytes(token_deltas[0].into());
        let token_1_delta = U256::from_be_bytes(token_deltas[1].into());

        let details = db_tx.get_protocol_details_sorted(info.target_address)?;
        let [token_0, token_1] = [details.token0, details.token1];

        let t0_info = db_tx.try_fetch_token_info(token_0)?;
        let t1_info = db_tx.try_fetch_token_info(token_1)?;

        let am0 = token_0_delta.to_scaled_rational(t0_info.decimals);
        let am1 = token_1_delta.to_scaled_rational(t1_info.decimals);

        Ok(NormalizedBurn {
            protocol: Protocol::LFJ,
            recipient: info.from_address,
            pool: info.target_address,
            trace_index: info.trace_idx,
            from: info.from_address,
            token: vec![t0_info, t1_info],
            amount: vec![am0, am1],
        })
    }
);
action_impl!(
    Protocol::LFJ,
    crate::LFJPair::collectProtocolFeesCall,
    Collect,
    [CollectedProtocolFees],
    call_data: true,
    return_data: true,
    logs: true,
    |
    info: CallInfo,
    call_data: collectProtocolFeesCall,
    return_data: collectProtocolFeesReturn,
    _logs: LFJCollectProtocolFeesCallLogs,
    db_tx: &DB
    | {
        let logs=_logs.collected_protocol_fees_field?;
        let details = db_tx.get_protocol_details_sorted(info.target_address)?;
        let [token_0, token_1] = [details.token0, details.token1];

        let t0_info = db_tx.try_fetch_token_info(token_0)?;
        let t1_info = db_tx.try_fetch_token_info(token_1)?;

        let collected_protocol_fees = return_data.collectedProtocolFees;

        // Extract the lower and upper 16 bytes from the 32-byte array
        let lower_16_bytes: [u8; 16] = collected_protocol_fees[0..16].try_into().expect("slice with incorrect length");
        let upper_16_bytes: [u8; 16] = collected_protocol_fees[16..32].try_into().expect("slice with incorrect length");

        // Convert each 16-byte array into a U256
        let lower_u256 = U256::from_be_bytes(lower_16_bytes);
        let upper_u256 = U256::from_be_bytes(upper_16_bytes);

        let am0 = lower_u256.to_scaled_rational(t0_info.decimals);
        let am1 = upper_u256.to_scaled_rational(t1_info.decimals);

        Ok(NormalizedCollect {
            protocol: Protocol::LFJ,
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient: logs.feeRecipient,
            pool: info.target_address,
            token: vec![t0_info, t1_info],
            amount: vec![am0, am1],
        })
    }
);

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy_primitives::{hex, Address, B256};
    use brontes_classifier::test_utils::ClassifierTestUtils;
    use brontes_types::{
        db::token_info::TokenInfoWithAddress, normalized_actions::Action, Protocol::LFJ,
        TreeSearchBuilder,
    };

    use super::*;

    #[brontes_macros::test]
    async fn test_lfj_swap() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let swap =
            B256::from(hex!("057f1d5b3ddabec1b8d78ac7181f562f755669494514f94a767247af800339b1"));

        let eq_action = Action::Swap(NormalizedSwap {
            protocol:    Protocol::LFJ,
            trace_index: 2,
            from:        Address::new(hex!("A69babEF1cA67A37Ffaf7a485DfFF3382056e78C")),
            recipient:   Address::new(hex!("A69babEF1cA67A37Ffaf7a485DfFF3382056e78C")),
            pool:        Address::new(hex!("88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640")),
            token_in:    TokenInfoWithAddress::weth(),
            amount_in:   U256::from_str("39283347298163243343")
                .unwrap()
                .to_scaled_rational(18),
            token_out:   TokenInfoWithAddress::usdc(),
            amount_out:  U256::from_str("98019119714").unwrap().to_scaled_rational(6),

            msg_value: U256::ZERO,
        });

        classifier_utils
            .contains_action(
                swap,
                0,
                eq_action,
                TreeSearchBuilder::default().with_action(Action::is_swap),
            )
            .await
            .unwrap();
    }

    #[brontes_macros::test]
    async fn test_lfj_mints() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let mint =
            B256::from(hex!("0089210683170b3f17201c8abeafdc4c022a26c7af1e44d351556eaa48d0fee8"));

        let eq_action = Action::Mint(NormalizedMint {
            protocol:    Protocol::LFJ,
            trace_index: 21,
            from:        Address::new(hex!("6b75d8AF000000e20B7a7DDf000Ba900b4009A80")),
            recipient:   Address::new(hex!("6b75d8AF000000e20B7a7DDf000Ba900b4009A80")),
            pool:        Address::new(hex!("3416cF6C708Da44DB2624D63ea0AAef7113527C6")),
            token:       vec![TokenInfoWithAddress::usdc(), TokenInfoWithAddress::usdt()],
            amount:      vec![
                U256::from_str("102642322850")
                    .unwrap()
                    .to_scaled_rational(6),
                U256::from_str("250137480130")
                    .unwrap()
                    .to_scaled_rational(6),
            ],
        });

        classifier_utils
            .contains_action(
                mint,
                0,
                eq_action,
                TreeSearchBuilder::default().with_action(Action::is_mint),
            )
            .await
            .unwrap();
    }

    #[brontes_macros::test]
    async fn test_lfj_burn() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let burn =
            B256::from(hex!("f179f349434a59d0dc899fc03a5754c7e50f52de1709d9523e7cbd09c4ba13eb"));

        let eq_action = Action::Burn(NormalizedBurn {
            protocol:    Protocol::LFJ,
            trace_index: 12,
            from:        Address::new(hex!("6b75d8AF000000e20B7a7DDf000Ba900b4009A80")),
            recipient:   Address::new(hex!("6b75d8AF000000e20B7a7DDf000Ba900b4009A80")),
            pool:        Address::new(hex!("3416cF6C708Da44DB2624D63ea0AAef7113527C6")),
            token:       vec![TokenInfoWithAddress::usdc(), TokenInfoWithAddress::usdt()],
            amount:      vec![
                U256::from_str("347057356182")
                    .unwrap()
                    .to_scaled_rational(6),
                U256::from_str("5793599811").unwrap().to_scaled_rational(6),
            ],
        });

        classifier_utils
            .contains_action(
                burn,
                0,
                eq_action,
                TreeSearchBuilder::default().with_action(Action::is_burn),
            )
            .await
            .unwrap();
    }

    #[brontes_macros::test]
    async fn test_lfj_collect() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let collect =
            B256::from(hex!("f179f349434a59d0dc899fc03a5754c7e50f52de1709d9523e7cbd09c4ba13eb"));

        let eq_action = Action::Collect(NormalizedCollect {
            protocol:    Protocol::LFJ,
            trace_index: 13,
            from:        Address::new(hex!("6b75d8AF000000e20B7a7DDf000Ba900b4009A80")),
            recipient:   Address::new(hex!("6b75d8AF000000e20B7a7DDf000Ba900b4009A80")),
            pool:        Address::new(hex!("3416cF6C708Da44DB2624D63ea0AAef7113527C6")),
            token:       vec![TokenInfoWithAddress::usdc(), TokenInfoWithAddress::usdt()],
            amount:      vec![
                U256::from_str("347081800129")
                    .unwrap()
                    .to_scaled_rational(6),
                U256::from_str("5793599811").unwrap().to_scaled_rational(6),
            ],
        });

        classifier_utils
            .contains_action(
                collect,
                0,
                eq_action,
                TreeSearchBuilder::default().with_action(Action::is_collect),
            )
            .await
            .unwrap();
    }
}
