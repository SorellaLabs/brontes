use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    normalized_actions::NormalizedSwap, structured_trace::CallInfo, ToScaledRational,
};

action_impl!(
    Protocol::CurveV2MetapoolImpl,
    crate::CurveV2MetapoolImpl::exchange_0Call,
    Swap,
    [..TokenExchange],
    logs: true,
    |
    info: CallInfo,
    log: CurveV2MetapoolImplexchange_0CallLogs,
    db_tx: &DB|{
        let log = log.TokenExchange_field;

        let details = db_tx.get_protocol_details(info.from_address)?;

        let token_in_addr = match log.sold_id {
            0 => details.token0,
            1 => details.curve_lp_token.ok_or(eyre::eyre!("Expected curve_lp_token for token in, found None"))?,
            _ => unreachable!()
        };

        let token_out_addr = match log.bought_id {
            0 => details.token0,
            1 => details.curve_lp_token.ok_or(eyre::eyre!("Expected curve_lp_token for token out, found None"))?,
            _ => unreachable!()
        };

        let token_in = db_tx.try_fetch_token_info(token_in_addr)?;
        let token_out = db_tx.try_fetch_token_info(token_out_addr)?;

        let amount_in = log.tokens_sold.to_scaled_rational(token_in.decimals);
        let amount_out = log.tokens_bought.to_scaled_rational(token_out.decimals);

        Ok(NormalizedSwap {
            protocol: details.protocol,
            pool: info.from_address,
            trace_index: info.trace_idx,
            from: info.msg_sender,
            recipient: info.msg_sender,
            token_in,
            token_out,
            amount_in,
            amount_out,
            msg_value: info.msg_value
        })
    }
);

action_impl!(
    Protocol::CurveV2MetapoolImpl,
    crate::CurveV2MetapoolImpl::exchange_underlying_0Call,
    Swap,
    [..TokenExchangeUnderlying],
    logs: true,
    |
    info: CallInfo,
    log: CurveV2MetapoolImplexchange_underlying_0CallLogs,
    db_tx: &DB|{
        let log = log.TokenExchangeUnderlying_field;

        let details = db_tx.get_protocol_details(info.from_address)?;

        let token_in_addr = match log.sold_id {
            0 => details.token0,
            1 => details.token1,
            2 => details.token2.ok_or(eyre::eyre!("Expected token2 for token in, found None"))?,
            3 => details.token3.ok_or(eyre::eyre!("Expected token3 for token in, found None"))?,
            4 => details.token4.ok_or(eyre::eyre!("Expected token4 for token in, found None"))?,
            _ => unreachable!()
        };

        let token_out_addr = match log.bought_id {
            0 => details.token0,
            1 => details.token1,
            2 => details.token2.ok_or(eyre::eyre!("Expected token2 for token out, found None"))?,
            3 => details.token3.ok_or(eyre::eyre!("Expected token3 for token out, found None"))?,
            4 => details.token4.ok_or(eyre::eyre!("Expected token4 for token out, found None"))?,
            _ => unreachable!()
        };

        let token_in = db_tx.try_fetch_token_info(token_in_addr)?;
        let token_out = db_tx.try_fetch_token_info(token_out_addr)?;

        let amount_in = log.tokens_sold.to_scaled_rational(token_in.decimals);
        let amount_out = log.tokens_bought.to_scaled_rational(token_out.decimals);


        Ok(NormalizedSwap {
            protocol: details.protocol,
            pool: info.from_address,
            trace_index: info.trace_idx,
            from: info.msg_sender,
            recipient: info.msg_sender,
            token_in,
            token_out,
            amount_in,
            amount_out,
            msg_value: info.msg_value
        })
    }
);

action_impl!(
    Protocol::CurveV2MetapoolImpl,
    crate::CurveV2MetapoolImpl::exchange_underlying_1Call,
    Swap,
    [..TokenExchangeUnderlying],
    logs: true,
    |
    info: CallInfo,
    log: CurveV2MetapoolImplexchange_underlying_1CallLogs,
    db_tx: &DB|{
        let log = log.TokenExchangeUnderlying_field;

        let details = db_tx.get_protocol_details(info.from_address)?;

        let token_in_addr = match log.sold_id {
            0 => details.token0,
            1 => details.token1,
            2 => details.token2.ok_or(eyre::eyre!("Expected token2 for token in, found None"))?,
            3 => details.token3.ok_or(eyre::eyre!("Expected token3 for token in, found None"))?,
            4 => details.token4.ok_or(eyre::eyre!("Expected token4 for token in, found None"))?,
            _ => unreachable!()
        };

        let token_out_addr = match log.bought_id {
            0 => details.token0,
            1 => details.token1,
            2 => details.token2.ok_or(eyre::eyre!("Expected token2 for token out, found None"))?,
            3 => details.token3.ok_or(eyre::eyre!("Expected token3 for token out, found None"))?,
            4 => details.token4.ok_or(eyre::eyre!("Expected token4 for token out, found None"))?,
            _ => unreachable!()
        };

        let token_in = db_tx.try_fetch_token_info(token_in_addr)?;
        let token_out = db_tx.try_fetch_token_info(token_out_addr)?;

        let amount_in = log.tokens_sold.to_scaled_rational(token_in.decimals);
        let amount_out = log.tokens_bought.to_scaled_rational(token_out.decimals);


        Ok(NormalizedSwap {
            protocol: details.protocol,
            pool: info.from_address,
            trace_index: info.trace_idx,
            from: info.msg_sender,
            recipient: info.msg_sender,
            token_in,
            token_out,
            amount_in,
            amount_out,
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
        db::token_info::{TokenInfo, TokenInfoWithAddress},
        normalized_actions::Actions,
        Node, NodeData, ToScaledRational, TreeSearchArgs,
    };

    use super::*;

    #[brontes_macros::test]
    async fn test_curve_v2_metapool_impl_exchange0() {
        let classifier_utils = ClassifierTestUtils::new().await;
        classifier_utils.ensure_protocol(
            Protocol::CurveV2MetaPool,
            Address::new(hex!("892D701d94a43bDBCB5eA28891DaCA2Fa22A690b")),
            Address::new(hex!("530824DA86689C9C17CdC2871Ff29B058345b44a")),
            Address::new(hex!("6B175474E89094C44Da98b954EedeAC495271d0F")),
            Some(Address::new(hex!(
                "A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
            ))),
            Some(Address::new(hex!(
                "dAC17F958D2ee523a2206206994597C13D831ec7"
            ))),
            None,
            Some(Address::new(hex!(
                "6c3F90f043a72FA612cbac8115EE7e52BDe6E490"
            ))),
        );

        let swap = B256::from(hex!(
            "c32dc9024f2680772ce9d6c153f4293085ee0bd5fe97f100566df0b89aec4d23"
        ));

        let token_in = TokenInfoWithAddress {
            address: Address::new(hex!("6c3F90f043a72FA612cbac8115EE7e52BDe6E490")),
            inner: TokenInfo {
                decimals: 18,
                symbol: "3Crv".to_string(),
            },
        };

        let token_out = TokenInfoWithAddress {
            address: Address::new(hex!("530824DA86689C9C17CdC2871Ff29B058345b44a")),
            inner: TokenInfo {
                decimals: 18,
                symbol: "STBT".to_string(),
            },
        };

        classifier_utils.ensure_token(token_in.clone());
        classifier_utils.ensure_token(token_out.clone());

        let eq_action = Actions::Swap(NormalizedSwap {
            protocol: Protocol::CurveV2MetaPool,
            trace_index: 1,
            from: Address::new(hex!("81BD585940501b583fD092BC8397F2119A96E5ba")),
            recipient: Address::new(hex!("81BD585940501b583fD092BC8397F2119A96E5ba")),
            pool: Address::new(hex!("892D701d94a43bDBCB5eA28891DaCA2Fa22A690b")),
            token_in,
            amount_in: U256::from_str("754647000000000000000000")
                .unwrap()
                .to_scaled_rational(18),
            token_out,
            amount_out: U256::from_str("770465351189286428927839")
                .unwrap()
                .to_scaled_rational(18),
            msg_value: U256::ZERO,
        });

        let search_fn = |node: &Node, data: &NodeData<Actions>| TreeSearchArgs {
            collect_current_node: data
                .get_ref(node.data)
                .map(|s| s.is_swap())
                .unwrap_or_default(),
            child_node_to_collect: node
                .get_all_sub_actions()
                .iter()
                .filter_map(|d| data.get_ref(*d))
                .any(|action| action.is_swap()),
        };

        classifier_utils
            .contains_action(swap, 0, eq_action, search_fn)
            .await
            .unwrap();
    }

    // #[brontes_macros::test]
    // async fn test_curve_v2_metapool_exchange_underlying1() {
    //     let classifier_utils = ClassifierTestUtils::new().await;
    //     classifier_utils.ensure_protocol(
    //         Protocol::CurveV2MetaPool,
    //         Address::new(hex!("892D701d94a43bDBCB5eA28891DaCA2Fa22A690b")),
    //         Address::new(hex!("530824DA86689C9C17CdC2871Ff29B058345b44a")),
    //         Address::new(hex!("6c3F90f043a72FA612cbac8115EE7e52BDe6E490")),
    //         None,
    //         None,
    //         None,
    //         None,
    //     );

    //     classifier_utils.ensure_protocol(
    //         Protocol::CurveBasePool,
    //         Address::new(hex!("bEbc44782C7dB0a1A60Cb6fe97d0b483032FF1C7")),
    //         Address::new(hex!("6B175474E89094C44Da98b954EedeAC495271d0F")),
    //         Address::new(hex!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")),
    //         Some(Address::new(hex!("dAC17F958D2ee523a2206206994597C13D831ec7"
    // ))),         None,
    //         None,
    //         None,
    //     );

    //     let three_crv = TokenInfoWithAddress {
    //         address:
    // Address::new(hex!("6c3F90f043a72FA612cbac8115EE7e52BDe6E490")),
    //         inner:   TokenInfo { decimals: 18, symbol: "3Crv".to_string() },
    //     };

    //     let swap =
    //         B256::from(hex!("
    // a835d77e510a6218199c44aa911ac0056ebbb339015c3a0d56c4020c5ca5a115"));

    //     let token_in = TokenInfoWithAddress {
    //         address:
    // Address::new(hex!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")),
    //         inner:   TokenInfo { decimals: 6, symbol: "USDC".to_string() },
    //     };

    //     let token_out = TokenInfoWithAddress {
    //         address:
    // Address::new(hex!("530824DA86689C9C17CdC2871Ff29B058345b44a")),
    //         inner:   TokenInfo { decimals: 18, symbol: "STBT".to_string() },
    //     };

    //     classifier_utils.ensure_token(token_in.clone());
    //     classifier_utils.ensure_token(token_out.clone());
    //     classifier_utils.ensure_token(three_crv.clone());

    //     let eq_action = Actions::Swap(NormalizedSwap {
    //         protocol: Protocol::CurveV2MetaPool,
    //         trace_index: 1,
    //         from:
    // Address::new(hex!("31b8939C6e55A4DDaF0d6479320A0DFD9766EE9D")),
    //         recipient:
    // Address::new(hex!("31b8939C6e55A4DDaF0d6479320A0DFD9766EE9D")),
    //         pool:
    // Address::new(hex!("892D701d94a43bDBCB5eA28891DaCA2Fa22A690b")),
    //         token_in,
    //         amount_in:
    // U256::from_str("500000000").unwrap().to_scaled_rational(6),
    //         token_out,
    //         amount_out: U256::from_str("500390219856882922498")
    //             .unwrap()
    //             .to_scaled_rational(18),
    //         msg_value: U256::ZERO,
    //     });

    //     let search_fn = |node: &Node<Actions>| TreeSearchArgs {
    //         collect_current_node:  node.data.is_swap(),
    //         child_node_to_collect: node
    //             .get_all_sub_actions()
    //             .iter()
    //             .any(|action| action.is_swap()),
    //     };

    //     classifier_utils
    //         .contains_action(swap, 0, eq_action, search_fn)
    //         .await
    //         .unwrap();
    // }
}
