#[cfg(test)]
mod cancel_stream {
    use crate::helpers::{
        mock_messages::{get_create_stream_msg, get_factory_inst_msg},
        setup::{setup, SetupResponse},
        utils::{get_contract_address_from_res, get_funds_from_res, get_wasm_attribute_with_key},
    };
    use cosmwasm_std::{coin, Addr, BlockInfo, Uint128};
    use cw_multi_test::Executor;
    use streamswap_stream::{msg::ExecuteMsg as StreamSwapExecuteMsg, ContractError};

    #[test]
    fn cancel_stream_error_unauthorized() {
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
            time: start_time.plus_seconds(20),
            height: 2,
            chain_id: "test".to_string(),
        });

        let subscribe_msg = StreamSwapExecuteMsg::Subscribe {
            operator_target: None,
            operator: None,
        };

        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &subscribe_msg,
                &[coin(100, "in_denom")],
            )
            .unwrap();

        // Creator cannot cancel stream, only protocol admin can
        let cancel_stream_msg = StreamSwapExecuteMsg::CancelStream {};

        let res = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &cancel_stream_msg,
                &[],
            )
            .unwrap_err();

        let err = res.source().unwrap();
        let error = err.downcast_ref::<ContractError>().unwrap();
        assert_eq!(*error, ContractError::Unauthorized {});
    }

    #[test]
    fn cancel_steam_error_without_pause() {
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
            time: start_time.plus_seconds(20),
            height: 2,
            chain_id: "test".to_string(),
        });

        let subscribe_msg = StreamSwapExecuteMsg::Subscribe {
            operator_target: None,
            operator: None,
        };

        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &subscribe_msg,
                &[coin(100, "in_denom")],
            )
            .unwrap();

        // Cannot cancel stream without pausing it first
        let cancel_stream_msg = StreamSwapExecuteMsg::CancelStream {};

        let res = app
            .execute_contract(
                test_accounts.admin.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &cancel_stream_msg,
                &[],
            )
            .unwrap_err();

        let err = res.source().unwrap();
        let error = err.downcast_ref::<ContractError>().unwrap();
        assert_eq!(*error, ContractError::StreamNotPaused {});
    }

    #[test]
    fn cancel_stream_happy_path() {
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
        let out_amount = coin(100, "out_denom");

        let create_stream_msg = get_create_stream_msg(
            "stream",
            None,
            &test_accounts.creator_1.to_string(),
            out_amount.clone(),
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
            time: start_time.plus_seconds(20),
            height: 2,
            chain_id: "test".to_string(),
        });

        let subscribe_msg = StreamSwapExecuteMsg::Subscribe {
            operator_target: None,
            operator: None,
        };

        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &subscribe_msg,
                &[coin(100, "in_denom")],
            )
            .unwrap();

        let pause_msg = StreamSwapExecuteMsg::PauseStream {};

        let _res = app
            .execute_contract(
                test_accounts.admin.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &pause_msg,
                &[],
            )
            .unwrap();

        let cancel_stream_msg = StreamSwapExecuteMsg::CancelStream {};

        let _res = app
            .execute_contract(
                test_accounts.admin.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &cancel_stream_msg,
                &[],
            )
            .unwrap();

        let stream_status = get_wasm_attribute_with_key(_res.clone(), "status".to_string());
        let fund_transfer = get_funds_from_res(_res.clone());

        assert_eq!(stream_status, "cancelled".to_string());
        assert_eq!(
            fund_transfer,
            vec![(test_accounts.creator_1.to_string(), out_amount.clone())]
        );
    }

    #[test]
    fn cancel_stream_error_already_cancelled() {
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
            time: start_time.plus_seconds(20),
            height: 2,
            chain_id: "test".to_string(),
        });

        let subscribe_msg = StreamSwapExecuteMsg::Subscribe {
            operator_target: None,
            operator: None,
        };

        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &subscribe_msg,
                &[coin(100, "in_denom")],
            )
            .unwrap();

        let pause_msg = StreamSwapExecuteMsg::PauseStream {};

        let _res = app
            .execute_contract(
                test_accounts.admin.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &pause_msg,
                &[],
            )
            .unwrap();

        let cancel_stream_msg = StreamSwapExecuteMsg::CancelStream {};

        let _res = app
            .execute_contract(
                test_accounts.admin.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &cancel_stream_msg,
                &[],
            )
            .unwrap();

        let res = app
            .execute_contract(
                test_accounts.admin.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &cancel_stream_msg,
                &[],
            )
            .unwrap_err();

        let err = res.source().unwrap();
        let error = err.downcast_ref::<ContractError>().unwrap();

        assert_eq!(*error, ContractError::StreamIsCancelled {});
    }

    #[test]
    fn cancel_stream_after_end_time() {
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
        let out_amount = coin(100, "out_denom");

        let create_stream_msg = get_create_stream_msg(
            "stream",
            None,
            &test_accounts.creator_1.to_string(),
            out_amount.clone(),
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
            time: start_time.plus_seconds(20),
            height: 2,
            chain_id: "test".to_string(),
        });

        let subscribe_msg = StreamSwapExecuteMsg::Subscribe {
            operator_target: None,
            operator: None,
        };

        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &subscribe_msg,
                &[coin(100, "in_denom")],
            )
            .unwrap();

        // Cannot pause stream after it has ended i.e., after end_time
        app.set_block(BlockInfo {
            time: end_time.plus_seconds(20),
            height: 3,
            chain_id: "test".to_string(),
        });

        let pause_msg = StreamSwapExecuteMsg::PauseStream {};

        let _res = app
            .execute_contract(
                test_accounts.admin.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &pause_msg,
                &[],
            )
            .unwrap_err();

        let err = _res.source().unwrap();
        let error = err.downcast_ref::<ContractError>().unwrap();
        assert_eq!(*error, ContractError::StreamEnded {});

        // Stream can be cancelled even after end time if it was paused before it ended
        app.set_block(BlockInfo {
            time: start_time.plus_seconds(20),
            height: 4,
            chain_id: "test".to_string(),
        });

        let _res = app
            .execute_contract(
                test_accounts.admin.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &pause_msg,
                &[],
            )
            .unwrap();

        app.set_block(BlockInfo {
            time: end_time.plus_seconds(20),
            height: 5,
            chain_id: "test".to_string(),
        });

        let cancel_stream_msg = StreamSwapExecuteMsg::CancelStream {};

        let _res = app
            .execute_contract(
                test_accounts.admin.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &cancel_stream_msg,
                &[],
            )
            .unwrap();

        let stream_status = get_wasm_attribute_with_key(_res.clone(), "status".to_string());
        let fund_transfer = get_funds_from_res(_res.clone());

        assert_eq!(stream_status, "cancelled".to_string());
        assert_eq!(
            fund_transfer,
            vec![(test_accounts.creator_1.to_string(), out_amount.clone())]
        );
    }
}
