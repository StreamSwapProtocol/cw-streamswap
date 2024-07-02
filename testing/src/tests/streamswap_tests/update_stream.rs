#[cfg(test)]
mod update_stream_tests {

    use std::str::FromStr;

    use crate::helpers::{
        mock_messages::{get_create_stream_msg, get_factory_inst_msg},
        setup::{setup, SetupResponse},
        utils::{get_contract_address_from_res, get_wasm_attribute_with_key},
    };
    use cosmwasm_std::VoteOption::No;
    use cosmwasm_std::{coin, Addr, BlockInfo, Decimal, Decimal256, Uint128};
    use cw_multi_test::Executor;
    use streamswap_stream::{
        msg::{ExecuteMsg as StreamSwapExecuteMsg, QueryMsg as StreamSwapQueryMsg, StreamResponse},
        state::Status,
        ContractError,
    };

    #[test]
    fn update_stream_without_subscription() {
        let SetupResponse {
            mut app,
            test_accounts,
            stream_swap_code_id,
            stream_swap_factory_code_id,
            vesting_code_id,
        } = setup();

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
            "stream",
            None,
            &test_accounts.creator_1.to_string(),
            coin(100, "out_denom"),
            "in_denom",
            app.block_info().time.plus_seconds(100).into(),
            app.block_info().time.plus_seconds(200).into(),
            Some(Uint128::from(100u128)),
            None,
            None,
        );

        let _res = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                factory_address.clone(),
                &create_stream_msg,
                &[coin(100, "fee_denom"), coin(100, "out_denom")],
            )
            .unwrap();

        let stream_swap_contract_address = get_contract_address_from_res(_res);

        let update_stream_msg = StreamSwapExecuteMsg::UpdateStream {};
        let res = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &update_stream_msg,
                &[],
            )
            .unwrap();
        let action = get_wasm_attribute_with_key(res.clone(), "action".to_string());
        let new_distribution_amount =
            get_wasm_attribute_with_key(res.clone(), "new_distribution_amount".to_string());
        let dist_index = get_wasm_attribute_with_key(res.clone(), "dist_index".to_string());

        assert_eq!(action, "update_stream");
        assert_eq!(new_distribution_amount, "0");
        assert_eq!(dist_index, "0");
    }

    #[test]
    fn update_stream_during_bootstraping_period() {
        let SetupResponse {
            mut app,
            test_accounts,
            stream_swap_code_id,
            stream_swap_factory_code_id,
            vesting_code_id,
        } = setup();

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

        let start_time = app.block_info().time.plus_seconds(100).into();
        let end_time = app.block_info().time.plus_seconds(200).into();

        let create_stream_msg = get_create_stream_msg(
            "stream",
            None,
            &test_accounts.creator_1.to_string(),
            coin(100, "out_denom"),
            "in_denom",
            start_time,
            end_time,
            Some(Uint128::from(100u128)),
            None,
            None,
        );

        let _res = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                factory_address.clone(),
                &create_stream_msg,
                &[coin(100, "fee_denom"), coin(100, "out_denom")],
            )
            .unwrap();

        let stream_swap_contract_address = get_contract_address_from_res(_res);

        let subscribe_msg = StreamSwapExecuteMsg::Subscribe {
            operator_target: None,
            operator: None,
        };

        let _res = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &subscribe_msg,
                &[coin(100, "in_denom")],
            )
            .unwrap();

        let action = get_wasm_attribute_with_key(_res.clone(), "action".to_string());

        assert_eq!(action, "subscribe_pending");

        let update_stream_msg = StreamSwapExecuteMsg::UpdateStream {};

        let _res = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &update_stream_msg,
                &[],
            )
            .unwrap();

        let stream: StreamResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &streamswap_stream::msg::QueryMsg::Stream {},
            )
            .unwrap();

        assert_eq!(stream.status, Status::Waiting);
        assert_eq!(stream.in_supply, Uint128::from(100u128));
        assert_eq!(stream.dist_index, Decimal256::zero());
        assert_eq!(stream.spent_in, Uint128::zero());

        app.set_block(BlockInfo {
            height: 1_100,
            time: start_time.plus_seconds(10),
            chain_id: "test".to_string(),
        });

        let _res = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &update_stream_msg,
                &[],
            )
            .unwrap();

        let stream: StreamResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &streamswap_stream::msg::QueryMsg::Stream {},
            )
            .unwrap();

        assert_eq!(stream.in_supply + stream.spent_in, Uint128::from(100u128));
        assert_eq!(stream.out_remaining, Uint128::from(90u128));
        assert_ne!(stream.dist_index, Decimal256::zero());
        assert_ne!(stream.spent_in, Uint128::zero());
    }

    #[test]
    fn update_stream_error_stream_paused() {
        let SetupResponse {
            mut app,
            test_accounts,
            stream_swap_code_id,
            stream_swap_factory_code_id,
            vesting_code_id,
        } = setup();

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

        let start_time = app.block_info().time.plus_seconds(100).into();
        let end_time = app.block_info().time.plus_seconds(200).into();

        let create_stream_msg = get_create_stream_msg(
            "stream",
            None,
            &test_accounts.creator_1.to_string(),
            coin(100, "out_denom"),
            "in_denom",
            start_time,
            end_time,
            Some(Uint128::from(100u128)),
            None,
            None,
        );

        let _res = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                factory_address.clone(),
                &create_stream_msg,
                &[coin(100, "fee_denom"), coin(100, "out_denom")],
            )
            .unwrap();

        let stream_swap_contract_address = get_contract_address_from_res(_res);

        app.set_block(BlockInfo {
            height: 1_100,
            time: start_time.plus_seconds(10),
            chain_id: "test".to_string(),
        });

        let pause_stream_msg = StreamSwapExecuteMsg::PauseStream {};
        let _res = app
            .execute_contract(
                test_accounts.admin.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &pause_stream_msg,
                &[],
            )
            .unwrap();

        let _res = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                Addr::unchecked(stream_swap_contract_address),
                &StreamSwapExecuteMsg::UpdateStream {},
                &[],
            )
            .unwrap_err();

        let err = _res.source().unwrap();
        let error = err.downcast_ref::<ContractError>().unwrap();

        assert_eq!(*error, ContractError::StreamPaused {});
    }
    #[test]
    fn test_price_feed() {
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
            coin(1_000, "out_denom"),
            "in_denom",
            start_time,
            end_time,
            None,
            None,
            None,
        );
        // Create Stream
        let res = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                factory_address.clone(),
                &create_stream_msg,
                &[coin(100, "fee_denom"), coin(1_000, "out_denom")],
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

        // Check current streamed price before update
        let stream: StreamResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapQueryMsg::Stream {},
            )
            .unwrap();
        assert_eq!(stream.current_streamed_price, Decimal::new(Uint128::new(0)));

        // First subscription
        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &subscribe_msg,
                &[coin(1_000, "in_denom")],
            )
            .unwrap();

        // Update environment to start_time plus 50 sec
        app.set_block(BlockInfo {
            height: 1_100,
            time: start_time.plus_seconds(50),
            chain_id: "test".to_string(),
        });
        // Update Stream
        let update_stream_msg = StreamSwapExecuteMsg::UpdateStream {};
        let _res = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &update_stream_msg,
                &[coin(100, "fee_denom")],
            )
            .unwrap();

        // Check current streamed price after update
        let stream: StreamResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapQueryMsg::Stream {},
            )
            .unwrap();
        assert_eq!(
            stream.current_streamed_price,
            Decimal::from_str("1").unwrap()
        );

        // Second subscription
        let _res = app
            .execute_contract(
                test_accounts.subscriber_2.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &subscribe_msg,
                &[coin(1_000, "in_denom")],
            )
            .unwrap();

        // Set time to start_time plus 75 secs
        app.set_block(BlockInfo {
            height: 1_100,
            time: start_time.plus_seconds(75),
            chain_id: "test".to_string(),
        });
        // Update Stream
        let update_stream_msg = StreamSwapExecuteMsg::UpdateStream {};
        let _res = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &update_stream_msg,
                &[coin(100, "fee_denom")],
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
            stream.current_streamed_price,
            Decimal::from_str("3").unwrap()
        );
    }
}
