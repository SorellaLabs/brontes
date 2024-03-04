use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    normalized_actions::NormalizedAggregator, structured_trace::CallInfo, utils::ToScaledRational,
};

use crate::OneInchAggregationRouterV5::{
    clipperSwapReturn, clipperSwapToReturn, clipperSwapToWithPermitReturn, fillOrderToReturn,
    swapReturn,
};

action_impl!(
    Protocol::OneInch,
    crate::OneInchAggregationRouterV5::swapCall,
    Aggregator,
    [],
    call_data: true,
    return_data: true,
    |
    info: CallInfo,
    call_data: swapCall,
    return_data: swapReturn,
    db_tx: &DB | {
        let dst_receiver = call_data.desc.dstReceiver;
        let token_in_amount = return_data.spentAmount;
        let token_out_amount = return_data.returnAmount;
        let token_in = db_tx.try_fetch_token_info(call_data.desc.srcToken)?;
        let token_out = db_tx.try_fetch_token_info(call_data.desc.dstToken)?;
        let amount_in = token_in_amount.to_scaled_rational(token_in.decimals);
        let amount_out = token_out_amount.to_scaled_rational(token_out.decimals);
        return Ok(NormalizedAggregator {
            protocol: Protocol::OneInch,
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient: dst_receiver,
            pool: info.target_address,
            token_in,
            token_out,
            amount_in,
            amount_out,
            child_actions: vec![],
            msg_value: info.msg_value
        })
    }
);

action_impl!(
    Protocol::OneInch,
    crate::OneInchAggregationRouterV5::fillOrderToCall,
    Aggregator,
    [..OrderFilled],
    call_data: true,
    return_data: true,
    |
    info: CallInfo,
    call_data: fillOrderToCall,
    return_data: fillOrderToReturn,
    db_tx: &DB | {
        let recipient = call_data.order_.receiver;
        let token_in_amount = return_data.actualMakingAmount;
        let token_out_amount = return_data.actualTakingAmount;
        let token_in = db_tx.try_fetch_token_info(call_data.order_.makerAsset)?;
        let token_out = db_tx.try_fetch_token_info(call_data.order_.takerAsset)?;
        let amount_in = token_in_amount.to_scaled_rational(token_in.decimals);
        let amount_out = token_out_amount.to_scaled_rational(token_out.decimals);
        return Ok(NormalizedAggregator {
            protocol: Protocol::OneInch,
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient,
            pool: info.target_address,
            token_in,
            token_out,
            amount_in,
            amount_out,
            child_actions: vec![],
            msg_value: info.msg_value
        })
    }
);

action_impl!(
    Protocol::OneInch,
    crate::OneInchAggregationRouterV5::clipperSwapCall,
    Aggregator,
    [],
    call_data: true,
    return_data: true,
    |
    info: CallInfo,
    call_data: clipperSwapCall,
    return_data: clipperSwapReturn,
    db_tx: &DB | {
        let token_in_amount = call_data.inputAmount;
        let token_out_amount = return_data.returnAmount;
        let token_in = db_tx.try_fetch_token_info(call_data.srcToken)?;
        let token_out = db_tx.try_fetch_token_info(call_data.dstToken)?;
        let amount_in = token_in_amount.to_scaled_rational(token_in.decimals);
        let amount_out = token_out_amount.to_scaled_rational(token_out.decimals);
        return Ok(NormalizedAggregator {
            protocol: Protocol::OneInch,
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient: info.msg_sender,
            pool: info.target_address,
            token_in,
            token_out,
            amount_in,
            amount_out,
            child_actions: vec![],
            msg_value: info.msg_value
        })
    }
);

action_impl!(
    Protocol::OneInch,
    crate::OneInchAggregationRouterV5::clipperSwapToCall,
    Aggregator,
    [],
    call_data: true,
    return_data: true,
    |
    info: CallInfo,
    call_data: clipperSwapToCall,
    return_data: clipperSwapToReturn,
    db_tx: &DB | {
        let recipient = call_data.recipient;
        let token_in_amount = call_data.inputAmount;
        let token_out_amount = return_data.returnAmount;
        let token_in = db_tx.try_fetch_token_info(call_data.srcToken)?;
        let token_out = db_tx.try_fetch_token_info(call_data.dstToken)?;
        let amount_in = token_in_amount.to_scaled_rational(token_in.decimals);
        let amount_out = token_out_amount.to_scaled_rational(token_out.decimals);
        return Ok(NormalizedAggregator {
            protocol: Protocol::OneInch,
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient,
            pool: info.target_address,
            token_in,
            token_out,
            amount_in,
            amount_out,
            child_actions: vec![],
            msg_value: info.msg_value
        })
    }
);

action_impl!(
    Protocol::OneInch,
    crate::OneInchAggregationRouterV5::clipperSwapToWithPermitCall,
    Aggregator,
    [],
    call_data: true,
    return_data: true,
    |
    info: CallInfo,
    call_data: clipperSwapToWithPermitCall,
    return_data: clipperSwapToWithPermitReturn,
    db_tx: &DB | {
        let recipient = call_data.recipient;
        let token_in_amount = call_data.inputAmount;
        let token_out_amount = return_data.returnAmount;
        let token_in = db_tx.try_fetch_token_info(call_data.srcToken)?;
        let token_out = db_tx.try_fetch_token_info(call_data.dstToken)?;
        let amount_in = token_in_amount.to_scaled_rational(token_in.decimals);
        let amount_out = token_out_amount.to_scaled_rational(token_out.decimals);
        return Ok(NormalizedAggregator {
            protocol: Protocol::OneInch,
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient,
            pool: info.target_address,
            token_in,
            token_out,
            amount_in,
            amount_out,
            child_actions: vec![],
            msg_value: info.msg_value
        })
    }
);

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy_primitives::{hex, Address, B256, U256};
    use brontes_classifier::test_utils::ClassifierTestUtils;
    use brontes_types::{
        db::token_info::TokenInfoWithAddress, normalized_actions::Actions, Protocol::OneInch,
        ToScaledRational, TreeSearchBuilder,
    };

    use super::*;

    #[brontes_macros::test]
    async fn test_one_inch_aggregator_swap() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let aggregator =
            B256::from(hex!("68603b7dce39738bc7aa9ce1cce39992965820ae39388a6d62db8d2db70132bb"));

        let eq_action = Actions::Aggregator(NormalizedAggregator {
            protocol:      OneInch,
            trace_index:   0,
            from:          Address::new(hex!("f4F8845ceDe63e79De1B2c3bbA395e8547FE4283")),
            recipient:     Address::new(hex!("f4F8845ceDe63e79De1B2c3bbA395e8547FE4283")),
            pool:          Address::new(hex!("1111111254EEB25477B68fb85Ed929f73A960582")),
            token_in:      TokenInfoWithAddress::usdc(),
            amount_in:     U256::from_str("126000000000")
                .unwrap()
                .to_scaled_rational(6),
            token_out:     TokenInfoWithAddress::usdt(),
            amount_out:    U256::from_str("125475168379")
                .unwrap()
                .to_scaled_rational(6),
            child_actions: vec![],

            msg_value: U256::ZERO,
        });

        classifier_utils
            .contains_action(
                aggregator,
                0,
                eq_action,
                TreeSearchBuilder::default().with_action(Actions::is_aggregator),
            )
            .await
            .unwrap();
    }
}
