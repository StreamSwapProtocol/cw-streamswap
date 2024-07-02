#[cfg(test)]
mod pause_protocol_admin {

    use std::str::FromStr;

    use crate::helpers::{
        mock_messages::{get_create_stream_msg, get_factory_inst_msg},
        setup::{setup, SetupResponse},
        utils::{get_contract_address_from_res, get_wasm_attribute_with_key},
    };
    use cosmwasm_std::{coin, Addr, BlockInfo, Decimal, Decimal256, Uint128};
    use cw_multi_test::Executor;
    use streamswap_stream::{
        msg::{
            self, ExecuteMsg as StreamSwapExecuteMsg, QueryMsg as StreamSwapQueryMsg,
            StreamResponse,
        },
        ContractError as StreamSwapError,
    };

    #[test]
    fn test_cant_pause_before_start() {
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
        let end_time = app.block_info().time.plus_seconds(500).into();

        let create_stream_msg = get_create_stream_msg(
            "stream",
            None,
            &test_accounts.creator_1.to_string(),
            coin(100, "out_denom"),
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
                &[coin(100, "fee_denom"), coin(100, "out_denom")],
            )
            .unwrap();
        let stream_swap_contract_address: String = get_contract_address_from_res(res);
        app.set_block(BlockInfo {
            height: 1_100,
            time: start_time.plus_seconds(100),
            chain_id: "test".to_string(),
        });
        let pause_stream_msg = StreamSwapExecuteMsg::PauseStream {};

        //can't pause before start time
        app.set_block(BlockInfo {
            height: 1_100,
            time: start_time.minus_seconds(100),
            chain_id: "test".to_string(),
        });
        let pause_stream_msg = StreamSwapExecuteMsg::PauseStream {};
        let res = app
            .execute_contract(
                test_accounts.admin.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &pause_stream_msg,
                &[],
            )
            .unwrap_err();
        let err = res.source().unwrap();
        let error = err.downcast_ref::<StreamSwapError>().unwrap();
        assert_eq!(*error, StreamSwapError::StreamNotStarted {});
    }

    #[test]
    fn test_cant_pause_after_end() {
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
        let end_time = app.block_info().time.plus_seconds(500).into();

        let create_stream_msg = get_create_stream_msg(
            "stream",
            None,
            &test_accounts.creator_1.to_string(),
            coin(100, "out_denom"),
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
                &[coin(100, "fee_denom"), coin(100, "out_denom")],
            )
            .unwrap();
        let stream_swap_contract_address: String = get_contract_address_from_res(res);

        let msg = StreamSwapExecuteMsg::Subscribe {
            operator_target: None,
            operator: Some("subscriber_2".to_string()),
        };
        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &msg,
                &[coin(100, "in_denom")],
            )
            .unwrap();

        app.set_block(BlockInfo {
            height: 1_100,
            time: start_time.plus_seconds(100),
            chain_id: "test".to_string(),
        });
        let pause_stream_msg = StreamSwapExecuteMsg::PauseStream {};

        //can't pause after end time
        app.set_block(BlockInfo {
            height: 1_100,
            time: end_time.plus_seconds(500),
            chain_id: "test".to_string(),
        });
        let pause_stream_msg = StreamSwapExecuteMsg::PauseStream {};
        let res = app
            .execute_contract(
                test_accounts.admin.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &pause_stream_msg,
                &[],
            )
            .unwrap_err();
        let err = res.source().unwrap();
        let error = err.downcast_ref::<StreamSwapError>().unwrap();
        assert_eq!(*error, StreamSwapError::StreamEnded {});
    }

    #[test]
    fn pause_stream_auth_checks() {
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
        let end_time = app.block_info().time.plus_seconds(500).into();

        let create_stream_msg = get_create_stream_msg(
            "stream",
            None,
            &test_accounts.creator_1.to_string(),
            coin(100, "out_denom"),
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
                &[coin(100, "fee_denom"), coin(100, "out_denom")],
            )
            .unwrap();
        let stream_swap_contract_address: String = get_contract_address_from_res(res);
        app.set_block(BlockInfo {
            height: 1_100,
            time: start_time.plus_seconds(100),
            chain_id: "test".to_string(),
        });
        let pause_stream_msg = StreamSwapExecuteMsg::PauseStream {};

        // non protocol admin can't pause
        let res = app
            .execute_contract(
                test_accounts.wrong_user.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &pause_stream_msg,
                &[],
            )
            .unwrap_err();
        let err = res.source().unwrap();
        let error = err.downcast_ref::<StreamSwapError>().unwrap();
        assert_eq!(*error, StreamSwapError::Unauthorized {});
        // Stream creator can not pause
        let res = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &pause_stream_msg,
                &[],
            )
            .unwrap_err();
        let err = res.source().unwrap();
        let error = err.downcast_ref::<StreamSwapError>().unwrap();
        assert_eq!(*error, StreamSwapError::Unauthorized {});
    }
    #[test]
    fn test_pause_happy_path() {
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
        let end_time = app.block_info().time.plus_seconds(500).into();

        let create_stream_msg = get_create_stream_msg(
            "stream",
            None,
            &test_accounts.creator_1.to_string(),
            coin(100, "out_denom"),
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
                &[coin(100, "fee_denom"), coin(100, "out_denom")],
            )
            .unwrap();
        let stream_swap_contract_address: String = get_contract_address_from_res(res);
        let msg = StreamSwapExecuteMsg::Subscribe {
            operator_target: None,
            operator: Some("subscriber_1".to_string()),
        };
        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &msg,
                &[coin(100, "in_denom")],
            )
            .unwrap();
        app.set_block(BlockInfo {
            height: 1_100,
            time: start_time.plus_seconds(103),
            chain_id: "test".to_string(),
        });
        let pause_stream_msg = StreamSwapExecuteMsg::PauseStream {};

        //protocol admin can pause
        let pause_stream_msg = StreamSwapExecuteMsg::PauseStream {};
        let _res = app
            .execute_contract(
                test_accounts.admin.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &pause_stream_msg,
                &[],
            )
            .unwrap();

        // can't paused if already paused
        app.set_block(BlockInfo {
            height: 1_100,
            time: start_time.plus_seconds(105),
            chain_id: "test".to_string(),
        });
        let pause_stream_msg = StreamSwapExecuteMsg::PauseStream {};
        let res = app
            .execute_contract(
                test_accounts.admin.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &pause_stream_msg,
                &[],
            )
            .unwrap_err();
        let err = res.source().unwrap();
        let error = err.downcast_ref::<StreamSwapError>().unwrap();
        assert_eq!(*error, StreamSwapError::StreamKillswitchActive {});

        //cant subscribe more
        app.set_block(BlockInfo {
            height: 1_100,
            time: start_time.plus_seconds(102),
            chain_id: "test".to_string(),
        });
        let msg = StreamSwapExecuteMsg::Subscribe {
            operator_target: None,
            operator: Some("subscriber_2".to_string()),
        };
        let res = app
            .execute_contract(
                test_accounts.subscriber_2.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &msg,
                &[coin(100, "in_denom")],
            )
            .unwrap_err();
        let err = res.source().unwrap();
        let error = err.downcast_ref::<StreamSwapError>().unwrap();
        assert_eq!(*error, StreamSwapError::StreamKillswitchActive {});

        // can't update stream
        app.set_block(BlockInfo {
            height: 1_100,
            time: start_time.plus_seconds(102),
            chain_id: "test".to_string(),
        });
        let update_stream_msg = StreamSwapExecuteMsg::UpdateStream {};
        let res = app
            .execute_contract(
                test_accounts.admin.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &update_stream_msg,
                &[],
            )
            .unwrap_err();
        let err = res.source().unwrap();
        let error = err.downcast_ref::<StreamSwapError>().unwrap();
        assert_eq!(*error, StreamSwapError::StreamPaused {});

        // can't update position
        let update_position_msg = StreamSwapExecuteMsg::UpdatePosition {
            operator_target: Some("subscriber_1".to_string()),
        };
        let res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &update_position_msg,
                &[],
            )
            .unwrap_err();
        let err = res.source().unwrap();
        let error = err.downcast_ref::<StreamSwapError>().unwrap();
        assert_eq!(*error, StreamSwapError::StreamPaused {});

        // can't withdraw
        let update_stream_msg = StreamSwapExecuteMsg::Withdraw {
            cap: Some(Uint128::new(100)),
            operator_target: None,
        };
        let res = app
            .execute_contract(
                test_accounts.admin.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &update_stream_msg,
                &[],
            )
            .unwrap_err();
        let err = res.source().unwrap();
        let error = err.downcast_ref::<StreamSwapError>().unwrap();
        assert_eq!(*error, StreamSwapError::StreamKillswitchActive {});

        // can't finalize stream
        app.set_block(BlockInfo {
            height: 1_100,
            time: end_time.plus_seconds(102),
            chain_id: "test".to_string(),
        });
        let finalise_stream_msg = StreamSwapExecuteMsg::FinalizeStream { new_treasury: None };
        let res = app
            .execute_contract(
                test_accounts.admin.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &finalise_stream_msg,
                &[],
            )
            .unwrap_err();

        let err = res.source().unwrap();
        let error = err.downcast_ref::<StreamSwapError>().unwrap();
        assert_eq!(*error, StreamSwapError::StreamKillswitchActive {});

        // can't exit
        let exit_stream_msg = StreamSwapExecuteMsg::ExitStream {
            operator_target: None,
            salt: None,
        };
        let res = app
            .execute_contract(
                test_accounts.admin.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &exit_stream_msg,
                &[],
            )
            .unwrap_err();

        let err = res.source().unwrap();
        let error = err.downcast_ref::<StreamSwapError>().unwrap();
        assert_eq!(*error, StreamSwapError::StreamKillswitchActive {});
    }
}
