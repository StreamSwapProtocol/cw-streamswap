#[cfg(test)]
mod rounding_leftover {

    use std::str::FromStr;

    use crate::helpers::utils::get_contract_address_from_res;
    use crate::helpers::{
        mock_messages::{get_create_stream_msg, get_factory_inst_msg},
        setup::{setup, SetupResponse},
    };
    use cosmwasm_std::Uint128;
    use cosmwasm_std::{coin, Addr, BlockInfo, Decimal256, Timestamp};
    use cw_multi_test::Executor;
    use streamswap_stream::msg::{
        ExecuteMsg as StreamSwapExecuteMsg, QueryMsg as StreamSwapQueryMsg,
    };
    use streamswap_stream::msg::{PositionResponse, StreamResponse};

    #[test]
    fn test_rounding_leftover() {
        let SetupResponse {
            mut app,
            test_accounts,
            stream_swap_code_id,
            stream_swap_factory_code_id,
            vesting_code_id,
        } = setup();

        let start_time = Timestamp::from_seconds(1_000_000);
        let end_time = Timestamp::from_seconds(5_000_000);

        let msg = get_factory_inst_msg(stream_swap_code_id, vesting_code_id, &test_accounts);

        let factory_address = app
            .instantiate_contract(
                stream_swap_factory_code_id,
                test_accounts.admin.clone(),
                &msg,
                &[],
                "Factory".to_string(),
                None,
            )
            .unwrap();

        let create_stream_msg = get_create_stream_msg(
            &"Stream Swap tests".to_string(),
            Some("https://sample.url".to_string()),
            &test_accounts.creator_1.to_string(),
            coin(1_000_000_000_000, "out_denom"),
            "in_denom",
            start_time,
            end_time,
            None,
            None,
            None,
        );

        let res = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                factory_address.clone(),
                &create_stream_msg,
                &[coin(100, "fee_denom"), coin(1_000_000_000_000, "out_denom")],
            )
            .unwrap();
        let stream_swap_contract_address: String = get_contract_address_from_res(res);

        app.set_block(BlockInfo {
            time: start_time.plus_seconds(100),
            height: 1,
            chain_id: "SS".to_string(),
        });
        // First subscription
        let subscribe_msg = StreamSwapExecuteMsg::Subscribe {
            operator_target: None,
            operator: None,
        };
        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &subscribe_msg,
                &[coin(1_000_000_000, "in_denom")],
            )
            .unwrap();

        // Second subscription
        app.set_block(BlockInfo {
            time: start_time.plus_seconds(100_000),
            height: 2,
            chain_id: "SS".to_string(),
        });
        let _res = app
            .execute_contract(
                test_accounts.subscriber_2.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &subscribe_msg,
                &[coin(3_000_000_000, "in_denom")],
            )
            .unwrap();

        // Update position of subscriber 1
        app.set_block(BlockInfo {
            time: start_time.plus_seconds(3_000_000),
            height: 3,
            chain_id: "SS".to_string(),
        });
        let update_position_msg = StreamSwapExecuteMsg::UpdatePosition {
            operator_target: None,
        };
        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &update_position_msg,
                &[],
            )
            .unwrap();

        let position_1: PositionResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapQueryMsg::Position {
                    owner: test_accounts.subscriber_1.to_string(),
                },
            )
            .unwrap();
        assert_eq!(
            position_1.index,
            Decimal256::from_str("202.813614449380587585").unwrap()
        );
        assert_eq!(position_1.purchased, Uint128::new(202_813_614_449));
        assert_eq!(position_1.spent, Uint128::new(749_993_750));
        assert_eq!(position_1.in_balance, Uint128::new(250_006_250));

        let stream: StreamResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapQueryMsg::Stream {},
            )
            .unwrap();

        assert_eq!(
            stream.dist_index,
            Decimal256::from_str("202.813614449380587585").unwrap()
        );

        // Update position of subscriber 2
        app.set_block(BlockInfo {
            time: start_time.plus_seconds(3_575_000),
            height: 4,
            chain_id: "SS".to_string(),
        });
        let _res = app
            .execute_contract(
                test_accounts.subscriber_2.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &update_position_msg,
                &[],
            )
            .unwrap();

        let position_2: PositionResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapQueryMsg::Position {
                    owner: test_accounts.subscriber_2.to_string(),
                },
            )
            .unwrap();
        assert_eq!(
            position_2.index,
            Decimal256::from_str("238.074595237060799266").unwrap()
        );
        assert_eq!(position_2.purchased, Uint128::new(655_672_748_445));
        assert_eq!(position_2.spent, Uint128::new(2_673_076_923));
        assert_eq!(position_2.in_balance, Uint128::new(326_923_077));

        let stream: StreamResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapQueryMsg::Stream {},
            )
            .unwrap();

        assert_eq!(
            stream.dist_index,
            Decimal256::from_str("238.074595237060799266").unwrap()
        );

        // Update position after stream ends
        app.set_block(BlockInfo {
            time: end_time.plus_seconds(1),
            height: 5,
            chain_id: "SS".to_string(),
        });

        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &update_position_msg,
                &[],
            )
            .unwrap();

        let stream: StreamResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapQueryMsg::Stream {},
            )
            .unwrap();

        assert_eq!(
            stream.dist_index,
            Decimal256::from_str("264.137059297637397644").unwrap()
        );
        assert_eq!(stream.in_supply, Uint128::zero());

        let position_1: PositionResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapQueryMsg::Position {
                    owner: test_accounts.subscriber_1.to_string(),
                },
            )
            .unwrap();
        assert_eq!(
            position_1.index,
            Decimal256::from_str("264.137059297637397644").unwrap()
        );
        assert_eq!(position_1.spent, Uint128::new(1_000_000_000));
        assert_eq!(position_1.in_balance, Uint128::zero());

        // Update position after stream ends
        app.set_block(BlockInfo {
            time: end_time.plus_seconds(1),
            height: 6,
            chain_id: "SS".to_string(),
        });

        let _res = app
            .execute_contract(
                test_accounts.subscriber_2.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &update_position_msg,
                &[],
            )
            .unwrap();

        let stream: StreamResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapQueryMsg::Stream {},
            )
            .unwrap();

        assert_eq!(
            stream.dist_index,
            Decimal256::from_str("264.137059297637397644").unwrap()
        );
        assert_eq!(stream.in_supply, Uint128::zero());

        let position_2: PositionResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapQueryMsg::Position {
                    owner: test_accounts.subscriber_2.to_string(),
                },
            )
            .unwrap();
        assert_eq!(
            position_2.index,
            Decimal256::from_str("264.137059297637397644").unwrap()
        );
        assert_eq!(position_2.spent, Uint128::new(3_000_000_000));
        assert_eq!(position_2.in_balance, Uint128::zero());

        assert_eq!(stream.out_remaining, Uint128::zero());
        assert_eq!(
            position_1
                .purchased
                .checked_add(position_2.purchased)
                .unwrap(),
            // 1 difference due to rounding
            stream.out_asset.amount.saturating_sub(Uint128::new(1u128))
        );
    }
}

//     #[test]
//     fn test_rounding_leftover() {
//         let treasury = Addr::unchecked("treasury");
//         let start = Timestamp::from_seconds(1_000_000);
//         let end = Timestamp::from_seconds(5_000_000);
//         let out_supply = Uint128::new(1_000_000_000_000);
//         let out_denom = "out_denom";

//         // instantiate
//         let mut deps = mock_dependencies();
//         let mut env = mock_env();
//         env.block.time = Timestamp::from_seconds(100);
//         let msg = crate::msg::InstantiateMsg {
//             min_stream_seconds: Uint64::new(1000),
//             min_seconds_until_start_time: Uint64::new(1000),
//             stream_creation_denom: "fee".to_string(),
//             stream_creation_fee: Uint128::new(100),
//             exit_fee_percent: Decimal::percent(1),
//             fee_collector: "collector".to_string(),
//             protocol_admin: "protocol_admin".to_string(),
//             accepted_in_denom: "in".to_string(),
//         };
//         instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

//         // create stream
//         let mut env = mock_env();
//         env.block.time = Timestamp::from_seconds(1);
//         let info = mock_info(
//             "creator",
//             &[
//                 Coin::new(out_supply.u128(), out_denom),
//                 Coin::new(100, "fee"),
//             ],
//         );
//         execute_create_stream(
//             deps.as_mut(),
//             env,
//             info,
//             treasury.to_string(),
//             "test".to_string(),
//             Some("https://sample.url".to_string()),
//             "in".to_string(),
//             out_denom.to_string(),
//             out_supply,
//             start,
//             end,
//             None,
//         )
//         .unwrap();

//         // first subscription
//         let mut env = mock_env();
//         env.block.time = start.plus_seconds(100);
//         let info = mock_info("creator1", &[Coin::new(1_000_000_000, "in")]);
//         let msg = crate::msg::ExecuteMsg::Subscribe {
//             stream_id: 1,
//             operator_target: None,
//             operator: None,
//         };
//         execute(deps.as_mut(), env, info, msg).unwrap();

//         // second subscription
//         let mut env = mock_env();
//         env.block.time = start.plus_seconds(100_000);
//         let info = mock_info("creator2", &[Coin::new(3_000_000_000, "in")]);
//         let msg = crate::msg::ExecuteMsg::Subscribe {
//             stream_id: 1,
//             operator_target: None,
//             operator: None,
//         };
//         execute(deps.as_mut(), env, info, msg).unwrap();

//         // update position creator1
//         let mut env = mock_env();
//         env.block.time = start.plus_seconds(3_000_000);
//         let info = mock_info("creator1", &[]);
//         execute_update_position(deps.as_mut(), env.clone(), info, 1, None).unwrap();

//         let position =
//             query_position(deps.as_ref(), env.clone(), 1, "creator1".to_string()).unwrap();
//         assert_eq!(
//             position.index,
//             Decimal256::from_str("202.813614449380587585").unwrap()
//         );
//         assert_eq!(position.purchased, Uint128::new(202_813_614_449));
//         assert_eq!(position.spent, Uint128::new(749_993_750));
//         assert_eq!(position.in_balance, Uint128::new(250_006_250));
//         let stream = query_stream(deps.as_ref(), env, 1).unwrap();
//         assert_eq!(
//             stream.dist_index,
//             Decimal256::from_str("202.813614449380587585").unwrap()
//         );

//         // update position creator2
//         let mut env = mock_env();
//         env.block.time = start.plus_seconds(3_575_000);
//         let info = mock_info("creator2", &[]);
//         execute_update_position(deps.as_mut(), env.clone(), info, 1, None).unwrap();

//         let position =
//             query_position(deps.as_ref(), env.clone(), 1, "creator2".to_string()).unwrap();
//         assert_eq!(
//             position.index,
//             Decimal256::from_str("238.074595237060799266").unwrap()
//         );
//         assert_eq!(position.purchased, Uint128::new(655672748445));
//         assert_eq!(position.spent, Uint128::new(2673076923));
//         assert_eq!(position.in_balance, Uint128::new(326923077));
//         let stream = query_stream(deps.as_ref(), env, 1).unwrap();
//         assert_eq!(
//             stream.dist_index,
//             Decimal256::from_str("238.074595237060799266").unwrap()
//         );

//         // update position after stream ends
//         let mut env = mock_env();
//         env.block.time = end.plus_seconds(1);
//         let info = mock_info("creator1", &[]);
//         execute_update_position(deps.as_mut(), env.clone(), info, 1, None).unwrap();
//         let stream = query_stream(deps.as_ref(), env.clone(), 1).unwrap();
//         assert_eq!(
//             stream.dist_index,
//             Decimal256::from_str("264.137059297637397644").unwrap()
//         );
//         assert_eq!(stream.in_supply, Uint128::zero());
//         let position1 = query_position(deps.as_ref(), env, 1, "creator1".to_string()).unwrap();
//         assert_eq!(
//             position1.index,
//             Decimal256::from_str("264.137059297637397644").unwrap()
//         );
//         assert_eq!(position1.spent, Uint128::new(1_000_000_000));
//         assert_eq!(position1.in_balance, Uint128::zero());

//         // update position after stream ends
//         let mut env = mock_env();
//         env.block.time = end.plus_seconds(1);
//         let info = mock_info("creator2", &[]);
//         execute_update_position(deps.as_mut(), env.clone(), info, 1, None).unwrap();
//         let stream = query_stream(deps.as_ref(), env.clone(), 1).unwrap();
//         assert_eq!(
//             stream.dist_index,
//             Decimal256::from_str("264.137059297637397644").unwrap()
//         );
//         assert_eq!(stream.in_supply, Uint128::zero());
//         let position2 = query_position(deps.as_ref(), env, 1, "creator2".to_string()).unwrap();
//         assert_eq!(
//             position2.index,
//             Decimal256::from_str("264.137059297637397644").unwrap()
//         );
//         assert_eq!(position2.spent, Uint128::new(3_000_000_000));
//         assert_eq!(position2.in_balance, Uint128::zero());

//         assert_eq!(stream.out_remaining, Uint128::zero());
//         assert_eq!(
//             position1
//                 .purchased
//                 .checked_add(position2.purchased)
//                 .unwrap(),
//             // 1 difference due to rounding
//             stream.out_supply.sub(Uint128::new(1u128))
//         );
//     }
