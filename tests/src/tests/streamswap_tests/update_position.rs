#[cfg(test)]
mod update_position {

    use std::str::FromStr;

    use crate::helpers::suite::SuiteBuilder;
    use crate::helpers::utils::get_contract_address_from_res;
    #[cfg(test)]
    use crate::helpers::{
        mock_messages::{get_create_stream_msg, get_factory_inst_msg},
        suite::Suite,
    };
    use cosmwasm_std::{coin, Addr, BlockInfo, Decimal256, Uint256};
    use cw_multi_test::Executor;
    use streamswap_types::stream::{
        ExecuteMsg as StreamSwapExecuteMsg, PositionResponse, QueryMsg as StreamSwapQueryMsg,
    };

    #[test]
    fn update_position() {
        let Suite {
            mut app,
            test_accounts,
            stream_swap_code_id,
            stream_swap_factory_code_id,
            vesting_code_id,
        } = SuiteBuilder::default().build();
        let start_time = app.block_info().time.plus_seconds(100);
        let end_time = app.block_info().time.plus_seconds(200);
        let bootstrapping_start_time = app.block_info().time.plus_seconds(50);

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
            "Stream Swap tests",
            None,
            test_accounts.creator_1.as_ref(),
            coin(1_000_000, "out_denom"),
            "in_denom",
            bootstrapping_start_time,
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

        // Set time to start time
        app.set_block(BlockInfo {
            height: 1_000,
            time: start_time,
            chain_id: "test".to_string(),
        });

        // Update position without any subscription
        let update_position_msg = StreamSwapExecuteMsg::UpdatePosition {};

        let _res = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &update_position_msg,
                &[],
            )
            .unwrap_err();

        // First subscription
        let subscribe_msg = StreamSwapExecuteMsg::Subscribe {};

        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &subscribe_msg,
                &[coin(1_000_000, "in_denom")],
            )
            .unwrap();

        // Update time so we can check the position
        app.set_block(BlockInfo {
            height: 1_100,
            time: start_time.plus_seconds(50),
            chain_id: "test".to_string(),
        });
        // Update position
        let update_position_msg = StreamSwapExecuteMsg::UpdatePosition {};
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
            Uint256::from(500_000u128),
            "Position purchased should be 500_000"
        );
        assert_eq!(
            position.spent,
            Uint256::from(500_000u128),
            "Position spent should be 500_000"
        );
        assert_eq!(
            position.in_balance,
            Uint256::from(500_000u128),
            "Position in balance should be 500_000"
        );
        // Update time so we can check the position
        app.set_block(BlockInfo {
            height: 1_200,
            time: start_time.plus_seconds(75),
            chain_id: "test".to_string(),
        });
        // Update position
        let update_position_msg = StreamSwapExecuteMsg::UpdatePosition {};
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
            Uint256::from(750_000u128),
            "Position purchased should be 750_000"
        );
        assert_eq!(
            position.spent,
            Uint256::from(750_000u128),
            "Position spent should be 750_000"
        );

        // Set time to end time
        app.set_block(BlockInfo {
            height: 1_300,
            time: end_time.plus_seconds(1),
            chain_id: "test".to_string(),
        });
        // Update position
        let update_position_msg = StreamSwapExecuteMsg::UpdatePosition {};
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
            Uint256::from(1_000_000u128),
            "Position purchased should be 1_000_000"
        );
        assert_eq!(
            position.spent,
            Uint256::from(1_000_000u128),
            "Position spent should be 1_000_000"
        );
        assert_eq!(
            position.in_balance,
            Uint256::zero(),
            "Position in balance should be 0"
        );

        // Even if time passes after the stream ends, the position should remain the same
        app.set_block(BlockInfo {
            height: 1_400,
            time: end_time.plus_seconds(200),
            chain_id: "test".to_string(),
        });
        // Update position
        let update_position_msg = StreamSwapExecuteMsg::UpdatePosition {};
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
            Uint256::from(1_000_000u128),
            "Position purchased should be 1_000_000"
        );
        assert_eq!(
            position.spent,
            Uint256::from(1_000_000u128),
            "Position spent should be 1_000_000"
        );

        // Exit stream
        let exit_stream_msg = StreamSwapExecuteMsg::ExitStream { salt: None };
        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &exit_stream_msg,
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
        assert_eq!(position.exit_date, app.block_info().time);
    }
}
