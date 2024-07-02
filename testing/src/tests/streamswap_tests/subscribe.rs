#[cfg(test)]
mod subscibe_test {

    use std::str::FromStr;

    use crate::helpers::utils::get_contract_address_from_res;
    #[cfg(test)]
    use crate::helpers::{
        mock_messages::{get_create_stream_msg, get_factory_inst_msg},
        setup::{setup, SetupResponse},
    };
    use cosmwasm_std::{coin, Addr, BlockInfo, Decimal256, Uint128};
    use cw_multi_test::Executor;
    use cw_utils::PaymentError;
    use streamswap_stream::{
        msg::{
            ExecuteMsg as StreamSwapExecuteMsg, PositionResponse, QueryMsg as StreamSwapQueryMsg,
            StreamResponse,
        },
        ContractError as StreamSwapError,
    };
    #[test]
    fn test_first_subcription() {
        let SetupResponse {
            mut app,
            test_accounts,
            stream_swap_code_id,
            stream_swap_factory_code_id,
            vesting_code_id,
        } = setup();
        let start_time = app.block_info().time.plus_seconds(100).into();
        let end_time = app.block_info().time.plus_seconds(200).into();

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
            None,
            &test_accounts.creator_1.to_string(),
            coin(1_000_000, "out_denom"),
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
                &[coin(100, "fee_denom"), coin(1_000_000, "out_denom")],
            )
            .unwrap();
        let stream_swap_contract_address: String = get_contract_address_from_res(res);

        let subscribe_msg = StreamSwapExecuteMsg::Subscribe {
            operator_target: None,
            operator: None,
        };
        app.set_block(BlockInfo {
            height: 1_100,
            time: start_time,
            chain_id: "test".to_string(),
        });

        // No funds
        let res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &subscribe_msg,
                &[],
            )
            .unwrap_err();
        let err = res.source().unwrap();
        let error = err.downcast_ref::<StreamSwapError>().unwrap();
        assert_eq!(error, &StreamSwapError::Payment(PaymentError::NoFunds {}));
        // Incorrect denom
        let res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &subscribe_msg,
                &[coin(100, "wrong_denom")],
            )
            .unwrap_err();
        let err = res.source().unwrap();
        let error = err.downcast_ref::<StreamSwapError>().unwrap();
        assert_eq!(
            error,
            &StreamSwapError::Payment(PaymentError::MissingDenom("in_denom".to_string()))
        );

        // Subscribe
        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &subscribe_msg,
                &[coin(150, "in_denom")],
            )
            .unwrap();

        // Query Stream
        let stream: StreamResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapQueryMsg::Stream {},
            )
            .unwrap();
        // First subscription should set the stream to active
        assert_eq!(stream.status, streamswap_stream::state::Status::Active);
        // Dist index should be zero because no distribution has been made until last update
        assert_eq!(stream.dist_index, Decimal256::zero());
        // In supply should be updated
        assert_eq!(stream.in_supply, Uint128::new(150));
        // Position should be updated
        let position: PositionResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapQueryMsg::Position {
                    owner: test_accounts.subscriber_1.to_string(),
                },
            )
            .unwrap();
        assert_eq!(position.index, Decimal256::zero());
        assert_eq!(position.in_balance, Uint128::new(150));
        assert_eq!(position.spent, Uint128::zero());

        // Update stream
        app.set_block(BlockInfo {
            height: 2_200,
            time: start_time.plus_seconds(20),
            chain_id: "test".to_string(),
        });
        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapExecuteMsg::UpdateStream {},
                &[],
            )
            .unwrap();
        // Dist index should be updated
        let stream: StreamResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapQueryMsg::Stream {},
            )
            .unwrap();
        assert_eq!(
            stream.dist_index,
            Decimal256::from_str("1333.333333333333333333").unwrap()
        );
        assert_eq!(stream.in_supply, Uint128::new(120));
        assert_eq!(stream.spent_in, Uint128::new(30));
        assert_eq!(stream.last_updated, start_time.plus_seconds(20));
        assert_eq!(stream.shares, Uint128::new(150));
    }
    #[test]
    fn test_recurring_subscribe() {
        let SetupResponse {
            mut app,
            test_accounts,
            stream_swap_code_id,
            stream_swap_factory_code_id,
            vesting_code_id,
        } = setup();
        let start_time = app.block_info().time.plus_seconds(100).into();
        let end_time = app.block_info().time.plus_seconds(200).into();

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
            None,
            &test_accounts.creator_1.to_string(),
            coin(1_000_000, "out_denom"),
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
                &[coin(100, "fee_denom"), coin(1_000_000, "out_denom")],
            )
            .unwrap();
        let stream_swap_contract_address: String = get_contract_address_from_res(res);

        let subscribe_msg = StreamSwapExecuteMsg::Subscribe {
            operator_target: None,
            operator: None,
        };
        app.set_block(BlockInfo {
            height: 1_100,
            time: start_time,
            chain_id: "test".to_string(),
        });

        // First subscription
        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &subscribe_msg,
                &[coin(150, "in_denom")],
            )
            .unwrap();

        // Query Stream
        let stream: StreamResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapQueryMsg::Stream {},
            )
            .unwrap();
        assert_eq!(stream.status, streamswap_stream::state::Status::Active);
        assert_eq!(stream.dist_index, Decimal256::zero());
        assert_eq!(stream.in_supply, Uint128::new(150));
        let position: PositionResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapQueryMsg::Position {
                    owner: test_accounts.subscriber_1.to_string(),
                },
            )
            .unwrap();
        assert_eq!(position.index, Decimal256::zero());
        assert_eq!(position.in_balance, Uint128::new(150));
        assert_eq!(position.spent, Uint128::zero());

        // Non-operator tries to increase subscription
        let res = app
            .execute_contract(
                test_accounts.wrong_user.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapExecuteMsg::Subscribe {
                    operator_target: Some(test_accounts.subscriber_1.to_string()),
                    operator: None,
                },
                &[coin(150, "in_denom")],
            )
            .unwrap_err();
        let err = res.source().unwrap();
        let error = err.downcast_ref::<StreamSwapError>().unwrap();
        assert_eq!(error, &StreamSwapError::Unauthorized {});

        // Subscriber increases subscription in same block_time
        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapExecuteMsg::Subscribe {
                    operator_target: None,
                    operator: None,
                },
                &[coin(150, "in_denom")],
            )
            .unwrap();

        // Query Stream
        let stream: StreamResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapQueryMsg::Stream {},
            )
            .unwrap();
        // There will be no distribution because the last update was in the same block
        assert_eq!(stream.dist_index, Decimal256::from_str("0").unwrap());

        let position: PositionResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapQueryMsg::Position {
                    owner: test_accounts.subscriber_1.to_string(),
                },
            )
            .unwrap();
        assert_eq!(position.index, Decimal256::from_str("0").unwrap());
        assert_eq!(position.in_balance, Uint128::new(300));

        // Now simulate a block update
        app.set_block(BlockInfo {
            height: 1_200,
            time: start_time.plus_seconds(1),
            chain_id: "test".to_string(),
        });

        // Subscriber increases subscription
        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapExecuteMsg::Subscribe {
                    operator_target: None,
                    operator: None,
                },
                &[coin(150, "in_denom")],
            )
            .unwrap();

        // Query Stream
        let stream: StreamResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapQueryMsg::Stream {},
            )
            .unwrap();
        // Distribution index should be updated
        assert_eq!(
            stream.dist_index,
            Decimal256::from_str("33.333333333333333333").unwrap()
        );
        assert_eq!(stream.in_supply, Uint128::new(447));
        let position: PositionResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapQueryMsg::Position {
                    owner: test_accounts.subscriber_1.to_string(),
                },
            )
            .unwrap();

        assert_eq!(
            position.index,
            Decimal256::from_str("33.333333333333333333").unwrap()
        );
        assert_eq!(position.in_balance, Uint128::new(447));
        assert_eq!(position.spent, Uint128::from(3u128));
    }

    #[test]
    fn test_subscribe_pending() {
        let SetupResponse {
            mut app,
            test_accounts,
            stream_swap_code_id,
            stream_swap_factory_code_id,
            vesting_code_id,
        } = setup();
        let start_time = app.block_info().time.plus_seconds(100).into();
        let end_time = app.block_info().time.plus_seconds(200).into();

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
            None,
            &test_accounts.creator_1.to_string(),
            coin(1_000_000, "out_denom"),
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
                &[coin(100, "fee_denom"), coin(1_000_000, "out_denom")],
            )
            .unwrap();
        let stream_swap_contract_address: String = get_contract_address_from_res(res);

        let subscribe_msg = StreamSwapExecuteMsg::Subscribe {
            operator_target: None,
            operator: None,
        };
        app.set_block(BlockInfo {
            height: 1_100,
            time: start_time.minus_seconds(100),
            chain_id: "test".to_string(),
        });

        // First subscription
        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &subscribe_msg,
                &[coin(150, "in_denom")],
            )
            .unwrap();

        // Query Stream
        let stream: StreamResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapQueryMsg::Stream {},
            )
            .unwrap();
        assert_eq!(stream.status, streamswap_stream::state::Status::Waiting);
        assert_eq!(stream.dist_index, Decimal256::zero());
        assert_eq!(stream.in_supply, Uint128::new(150));
        let position: PositionResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapQueryMsg::Position {
                    owner: test_accounts.subscriber_1.to_string(),
                },
            )
            .unwrap();
        assert_eq!(position.index, Decimal256::zero());

        // Update stream
        app.set_block(BlockInfo {
            height: 1_200,
            time: start_time.minus_seconds(50),
            chain_id: "test".to_string(),
        });
        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapExecuteMsg::UpdateStream {},
                &[],
            )
            .unwrap();
        // Dist index should not be updated as the stream is still pending
        let stream: StreamResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapQueryMsg::Stream {},
            )
            .unwrap();
        assert_eq!(stream.dist_index, Decimal256::from_str("0").unwrap());
        assert_eq!(stream.in_supply, Uint128::new(150));
        assert_eq!(stream.spent_in, Uint128::zero());
        // Stream is still waiting so last updated should be the same as start time
        assert_eq!(stream.last_updated, start_time);
        assert_eq!(stream.shares, Uint128::new(150));

        // Subscriber increases subscription
        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapExecuteMsg::Subscribe {
                    operator_target: None,
                    operator: None,
                },
                &[coin(150, "in_denom")],
            )
            .unwrap();

        // Query Stream
        let stream: StreamResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapQueryMsg::Stream {},
            )
            .unwrap();
        // Distribution index should not be updated because we are still in pending status
        assert_eq!(stream.dist_index, Decimal256::from_str("0").unwrap());

        // Before stream start time, 2 subscriptions have been made and the stream is pending
        // Both subscriptions are made by the same user
        // At 10th second after start time, third subscription is made
        // This one is made by a different user
        // Stream is active
        // Total out supply is 1_000_000
        // Dist index should be = diff = 1/10  Dist balance  = 1000*1/10 = 100
        // Dist index = 100_000/300 = 333.333333333333333333
        // At 60th second after start time
        // Diff = 60-10 = 50/100-10 = 50/90 = 5/9 = 0.555555555555555555
        // Dist balance = 1_000_000*0.555555555555555555 = 555555.555555555555555
        // Dist index = 333.333+ 555555.555555555555555/466 = 1406.292560801144492131

        // Set time to start time plus 10 seconds
        app.set_block(BlockInfo {
            height: 1_300,
            time: start_time.plus_seconds(10),
            chain_id: "test".to_string(),
        });

        // Third subscription
        let _res = app
            .execute_contract(
                test_accounts.subscriber_2.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapExecuteMsg::Subscribe {
                    operator_target: None,
                    operator: None,
                },
                &[coin(150, "in_denom")],
            )
            .unwrap();
        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapExecuteMsg::UpdateStream {},
                &[],
            )
            .unwrap();

        // Dist index should be updated
        let stream: StreamResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapQueryMsg::Stream {},
            )
            .unwrap();
        assert_eq!(
            stream.dist_index,
            Decimal256::from_str("333.333333333333333333").unwrap()
        );

        // Set time to start time plus 60 seconds
        app.set_block(BlockInfo {
            height: 1_400,
            time: start_time.plus_seconds(60),
            chain_id: "test".to_string(),
        });

        // Update stream
        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapExecuteMsg::UpdateStream {},
                &[],
            )
            .unwrap();

        // Query Stream
        let stream: StreamResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapQueryMsg::Stream {},
            )
            .unwrap();
        // Distribution index should be updated
        assert_eq!(
            stream.dist_index,
            Decimal256::from_str("1406.292560801144492131").unwrap()
        );

        // Update position
        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapExecuteMsg::UpdatePosition {
                    operator_target: None,
                },
                &[],
            )
            .unwrap();

        // Query Position for subscriber 1
        let position: PositionResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapQueryMsg::Position {
                    owner: test_accounts.subscriber_1.to_string(),
                },
            )
            .unwrap();
        // Position should be updated
        assert_eq!(
            position.index,
            Decimal256::from_str("1406.292560801144492131").unwrap()
        );
        assert_eq!(position.in_balance, Uint128::new(120));
        assert_eq!(position.spent, Uint128::new(180));
    }
}
