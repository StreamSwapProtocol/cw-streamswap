#[cfg(test)]
mod resume_protocol_admin {

    use std::str::FromStr;

    use crate::helpers::{
        mock_messages::{get_create_stream_msg, get_factory_inst_msg},
        setup::{setup, SetupResponse},
        utils::{get_contract_address_from_res, get_wasm_attribute_with_key},
    };
    use cosmwasm_std::{coin, Addr, BlockInfo, Decimal, Decimal256, Uint128};
    use cw_multi_test::Executor;
    use streamswap_stream::state::Status;
    use streamswap_stream::{
        msg::{
            self, ExecuteMsg as StreamSwapExecuteMsg, QueryMsg as StreamSwapQueryMsg,
            StreamResponse,
        },
        ContractError as StreamSwapError,
    };
    #[test]
    fn resume_protocol() {
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
            time: start_time.plus_seconds(100),
            chain_id: "test".to_string(),
        });

        // non protocol admin can't pause
        let pause_stream_msg = StreamSwapExecuteMsg::PauseStream {};
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

        // protocol admin can Pause stream
        app.set_block(BlockInfo {
            height: 1_100,
            time: start_time.plus_seconds(101),
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

        // non protocol admin can't resume
        let resume_stream_msg = StreamSwapExecuteMsg::ResumeStream {};
        let res = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &resume_stream_msg,
                &[],
            )
            .unwrap_err();
        let err = res.source().unwrap();
        let error = err.downcast_ref::<StreamSwapError>().unwrap();
        assert_eq!(*error, StreamSwapError::Unauthorized {});

        // protocol admin can resume
        app.set_block(BlockInfo {
            height: 1_100,
            time: start_time.plus_seconds(103),
            chain_id: "test".to_string(),
        });
        let resume_stream_msg = StreamSwapExecuteMsg::ResumeStream {};
        let _res = app
            .execute_contract(
                test_accounts.admin.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &resume_stream_msg,
                &[],
            )
            .unwrap();
        // query stream
        let stream: StreamResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapQueryMsg::Stream {},
            )
            .unwrap();
        assert_eq!(stream.status, Status::Active);

        // can subscribe new after resume
        app.set_block(BlockInfo {
            height: 1_100,
            time: start_time.plus_seconds(104),
            chain_id: "test".to_string(),
        });
        let msg = StreamSwapExecuteMsg::Subscribe {
            operator_target: None,
            operator: None,
        };
        let res = app
            .execute_contract(
                test_accounts.subscriber_2.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &msg,
                &[coin(100, "in_denom")],
            )
            .unwrap();
        let action = get_wasm_attribute_with_key(res.clone(), "action".to_string());
        assert_eq!(action, "subscribe");
    }

    #[test]
    fn resume_failed_checks() {
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
            time: start_time.plus_seconds(100),
            chain_id: "test".to_string(),
        });

        // can't resume if not paused
        app.set_block(BlockInfo {
            height: 1_100,
            time: start_time.plus_seconds(103),
            chain_id: "test".to_string(),
        });
        let resume_stream_msg = StreamSwapExecuteMsg::ResumeStream {};
        let res = app
            .execute_contract(
                test_accounts.admin.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &resume_stream_msg,
                &[],
            )
            .unwrap_err();
        let err = res.source().unwrap();
        let error = err.downcast_ref::<StreamSwapError>().unwrap();
        assert_eq!(*error, StreamSwapError::StreamNotPaused {});

        // protocol admin can pause the stream
        app.set_block(BlockInfo {
            height: 1_100,
            time: start_time.plus_seconds(105),
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

        // cancel the stream
        app.set_block(BlockInfo {
            height: 1_100,
            time: start_time.plus_seconds(106),
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

        // can't resume if cancelled
        app.set_block(BlockInfo {
            height: 1_100,
            time: start_time.plus_seconds(107),
            chain_id: "test".to_string(),
        });
        let resume_stream_msg = StreamSwapExecuteMsg::ResumeStream {};
        let res = app
            .execute_contract(
                test_accounts.admin.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &resume_stream_msg,
                &[],
            )
            .unwrap_err();
        let err = res.source().unwrap();
        let error = err.downcast_ref::<StreamSwapError>().unwrap();
        assert_eq!(*error, StreamSwapError::StreamIsCancelled {});
    }
}
