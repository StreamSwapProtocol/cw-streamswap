#[cfg(test)]
mod update_position_tests {

    use std::str::FromStr;

    use crate::helpers::utils::get_contract_address_from_res;
    #[cfg(test)]
    use crate::helpers::{
        mock_messages::{get_create_stream_msg, get_factory_inst_msg},
        setup::{setup, SetupResponse},
    };
    use cosmwasm_std::{coin, Addr, BlockInfo, Decimal256, Uint128};
    use cw_multi_test::Executor;
    use streamswap_stream::{
        msg::{
            ExecuteMsg as StreamSwapExecuteMsg, PositionResponse, QueryMsg as StreamSwapQueryMsg,
        },
        ContractError as StreamSwapError,
    };

    #[test]
    fn update_position() {
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

        // Update position without any subscription
        let update_position_msg = StreamSwapExecuteMsg::UpdatePosition {
            operator_target: None,
        };

        let _res = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &update_position_msg,
                &[],
            )
            .unwrap_err();

        // First subscription
        let subscribe_msg = StreamSwapExecuteMsg::Subscribe {
            operator_target: None,
            operator: Some(test_accounts.subscriber_2.to_string()),
        };

        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &subscribe_msg,
                &[coin(1_000_000, "in_denom")],
            )
            .unwrap();

        // Non operator cannot update position
        let update_position_msg = StreamSwapExecuteMsg::UpdatePosition {
            operator_target: Some(test_accounts.subscriber_1.to_string()),
        };
        let res = app
            .execute_contract(
                test_accounts.wrong_user.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &update_position_msg,
                &[],
            )
            .unwrap_err();
        let error = res.downcast::<StreamSwapError>().unwrap();
        assert_eq!(error, StreamSwapError::Unauthorized {});

        // Operator can update position
        let update_position_msg = StreamSwapExecuteMsg::UpdatePosition {
            operator_target: Some(test_accounts.subscriber_1.to_string()),
        };
        let _res = app
            .execute_contract(
                test_accounts.subscriber_2.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &update_position_msg,
                &[],
            )
            .unwrap();
        // Update time so we can check the position
        app.set_block(BlockInfo {
            height: 1_100,
            time: start_time.plus_seconds(50),
            chain_id: "test".to_string(),
        });
        // Update position
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
        // We are at half of the stream time, so the position should be spent/remaning 50%
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
            Decimal256::from_str("0.5").unwrap(),
            "Position index should be 0.5"
        );
        assert_eq!(
            position.purchased,
            Uint128::new(500_000),
            "Position purchased should be 500_000"
        );
        assert_eq!(
            position.spent,
            Uint128::new(500_000),
            "Position spent should be 500_000"
        );
        assert_eq!(
            position.in_balance,
            Uint128::new(500_000),
            "Position in balance should be 500_000"
        );
        // Update time so we can check the position
        app.set_block(BlockInfo {
            height: 1_200,
            time: start_time.plus_seconds(75),
            chain_id: "test".to_string(),
        });
        // Update position
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
        // We are at 75% of the stream time, so the position should be spent/remaning 25%
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
            Decimal256::from_str("0.75").unwrap(),
            "Position index should be 0.75"
        );
        assert_eq!(
            position.purchased,
            Uint128::new(750_000),
            "Position purchased should be 750_000"
        );
        assert_eq!(
            position.spent,
            Uint128::new(750_000),
            "Position spent should be 750_000"
        );

        // Set time to end time
        app.set_block(BlockInfo {
            height: 1_300,
            time: end_time.plus_seconds(1),
            chain_id: "test".to_string(),
        });
        // Update position
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
        // We are at the end of the stream time, so the position should be spent/remaning 0%
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
            Decimal256::from_str("1").unwrap(),
            "Position index should be 1"
        );
        assert_eq!(
            position.purchased,
            Uint128::new(1_000_000),
            "Position purchased should be 1_000_000"
        );
        assert_eq!(
            position.spent,
            Uint128::new(1_000_000),
            "Position spent should be 1_000_000"
        );
        assert_eq!(
            position.in_balance,
            Uint128::zero(),
            "Position in balance should be 0"
        );

        // Even if time passes after the stream ends, the position should remain the same
        app.set_block(BlockInfo {
            height: 1_400,
            time: end_time.plus_seconds(200),
            chain_id: "test".to_string(),
        });
        // Update position
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
            Decimal256::from_str("1").unwrap(),
            "Position index should be 1"
        );
        assert_eq!(
            position.purchased,
            Uint128::new(1_000_000),
            "Position purchased should be 1_000_000"
        );
        assert_eq!(
            position.spent,
            Uint128::new(1_000_000),
            "Position spent should be 1_000_000"
        );

        // Exit stream
        let exit_stream_msg = StreamSwapExecuteMsg::ExitStream {
            operator_target: None,
            salt: None,
        };
        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &exit_stream_msg,
                &[],
            )
            .unwrap();
        // Update position
        let update_position_msg = StreamSwapExecuteMsg::UpdatePosition {
            operator_target: None,
        };
        // Position is deleted after exiting the stream
        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &update_position_msg,
                &[],
            )
            .unwrap_err();
    }
}
