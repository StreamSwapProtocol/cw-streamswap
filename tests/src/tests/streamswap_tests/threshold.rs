#[cfg(test)]
mod threshold {

    use crate::helpers::suite::SuiteBuilder;
    use crate::helpers::utils::{get_contract_address_from_res, get_funds_from_res};
    use crate::helpers::{
        mock_messages::{get_create_stream_msg, get_factory_inst_msg},
        suite::Suite,
    };
    use cosmwasm_std::testing::MockStorage;
    use cosmwasm_std::{coin, Addr, BlockInfo, Coin, Timestamp, Uint128, Uint256};
    use cw_multi_test::Executor;
    use streamswap_stream::ContractError as StreamSwapError;
    use streamswap_types::stream::{
        ExecuteMsg as StreamSwapExecuteMsg, QueryMsg as StreamSwapQueryMsg, StreamResponse,
        ThresholdError,
    };
    use streamswap_types::stream::{Status, Stream, ThresholdState};

    #[test]
    fn thresholds_state() {
        let mut storage = MockStorage::new();
        let thresholds = ThresholdState::new();
        let mut stream = Stream::new(
            Timestamp::from_seconds(0),
            "test".to_string(),
            Addr::unchecked("treasury"),
            Addr::unchecked("stream_admin"),
            Some("url".to_string()),
            Coin {
                denom: "out_denom".to_string(),
                amount: Uint128::from(100u128),
            },
            "in_denom".to_string(),
            Timestamp::from_seconds(0),
            Timestamp::from_seconds(100),
            Timestamp::from_seconds(0),
            None,
            None,
        );
        let threshold = Uint256::from(1_500_000_000_000u128);

        thresholds
            .set_threshold_if_any(Some(threshold), &mut storage)
            .unwrap();

        stream.spent_in = Uint256::from(1_500_000_000_000u128 - 1u128);
        let result = thresholds.error_if_not_reached(&storage, &stream.clone());
        assert!(result.is_err());
        stream.spent_in = Uint256::from(1_500_000_000_000u128);
        let result = thresholds.error_if_not_reached(&storage, &stream.clone());
        assert!(result.is_ok());
    }
    #[test]
    fn threshold_reached() {
        let Suite {
            mut app,
            test_accounts,
            stream_swap_code_id,
            stream_swap_factory_code_id,
            vesting_code_id,
        } = SuiteBuilder::default().build();
        let start_time = app.block_info().time.plus_seconds(1_000_000);
        let end_time = app.block_info().time.plus_seconds(5_000_000);
        let bootstrapping_start_time = app.block_info().time.plus_seconds(500_000);
        let threshold = Uint256::from(250u128);

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
            Some("https://sample.url".to_string()),
            test_accounts.creator_1.as_ref(),
            coin(500, "out_denom"),
            "in_denom",
            bootstrapping_start_time,
            start_time,
            end_time,
            Some(threshold),
            None,
            None,
        );

        let res = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                factory_address.clone(),
                &create_stream_msg,
                &[coin(100, "fee_denom"), coin(500, "out_denom")],
            )
            .unwrap();
        let stream_swap_contract_address: String = get_contract_address_from_res(res);

        let subscribe_msg = StreamSwapExecuteMsg::Subscribe {};
        // Set time to start of the stream
        app.set_block(BlockInfo {
            time: start_time,
            height: 1_000,
            chain_id: "test".to_string(),
        });

        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &subscribe_msg,
                &[coin(500, "in_denom")],
            )
            .unwrap();

        // Set block to the end of the stream
        app.set_block(BlockInfo {
            time: end_time.plus_seconds(1),
            height: 1_000,
            chain_id: "test".to_string(),
        });

        // Threshold should be reached
        let exit_msg = StreamSwapExecuteMsg::ExitStream { salt: None };

        let res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &exit_msg,
                &[],
            )
            .unwrap();

        // Exit should be possible
        // Since there is only one subscriber all out denom should be sent to subscriber
        let funds = get_funds_from_res(res);
        assert_eq!(
            Uint256::from(funds[0].1.amount.u128()),
            Uint256::from(500u128)
        );
        assert_eq!(funds[0].1.denom, "out_denom".to_string());

        // Creator finalizes the stream
        let finalize_msg = StreamSwapExecuteMsg::FinalizeStream { new_treasury: None };

        let res = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &finalize_msg,
                &[],
            )
            .unwrap();

        // Creator's revenue
        let funds = get_funds_from_res(res);
        assert_eq!(
            Uint256::from(funds[0].1.amount.u128()),
            Uint256::from(495u128)
        );
        assert_eq!(funds[0].1.denom, "in_denom".to_string());
        // Fee collector's revenue
        assert_eq!(
            Uint256::from(funds[1].1.amount.u128()),
            Uint256::from(5u128)
        );
        assert_eq!(funds[1].1.denom, "in_denom".to_string());
    }

    #[test]
    fn threshold_not_reached() {
        let Suite {
            mut app,
            test_accounts,
            stream_swap_code_id,
            stream_swap_factory_code_id,
            vesting_code_id,
        } = SuiteBuilder::default().build();
        let start_time = app.block_info().time.plus_seconds(1_000_000);
        let end_time = app.block_info().time.plus_seconds(5_000_000);
        let bootstrapping_start_time = app.block_info().time.plus_seconds(500_000);
        let threshold = Uint256::from(500u128);

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
            Some("https://sample.url".to_string()),
            test_accounts.creator_1.as_ref(),
            coin(500, "out_denom"),
            "in_denom",
            bootstrapping_start_time,
            start_time,
            end_time,
            Some(threshold),
            None,
            None,
        );

        let res = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                factory_address.clone(),
                &create_stream_msg,
                &[coin(100, "fee_denom"), coin(500, "out_denom")],
            )
            .unwrap();
        let stream_swap_contract_address: String = get_contract_address_from_res(res);

        // Set time to start of the stream
        app.set_block(BlockInfo {
            time: start_time,
            height: 1_000,
            chain_id: "test".to_string(),
        });

        let subscribe_msg = StreamSwapExecuteMsg::Subscribe {};
        // Subscription 1
        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &subscribe_msg,
                &[coin(250, "in_denom")],
            )
            .unwrap();
        // Subscription 2
        let _res = app
            .execute_contract(
                test_accounts.subscriber_2.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &subscribe_msg,
                &[coin(1, "in_denom")],
            )
            .unwrap();

        // Set block to the end of the stream
        app.set_block(BlockInfo {
            time: end_time.plus_seconds(1),
            height: 1_000,
            chain_id: "test".to_string(),
        });

        // Exit should not be possible
        let exit_msg = StreamSwapExecuteMsg::ExitStream { salt: None };

        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &exit_msg,
                &[],
            )
            .unwrap_err();

        // Finalize should not be possible
        let finalize_msg = StreamSwapExecuteMsg::FinalizeStream { new_treasury: None };

        let err = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &finalize_msg,
                &[],
            )
            .unwrap_err();
        let error = err.downcast::<StreamSwapError>().unwrap();
        assert_eq!(
            error,
            StreamSwapError::ThresholdError(ThresholdError::ThresholdNotReached {})
        );

        // Subscriber one executes exit cancelled before creator cancels stream
        let exit_cancelled_msg = StreamSwapExecuteMsg::ExitCancelled {};
        // Subscriber 1 executes exit cancelled
        let res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &exit_cancelled_msg,
                &[],
            )
            .unwrap();
        let subscriber_1_funds = get_funds_from_res(res);
        assert_eq!(subscriber_1_funds.len(), 1);
        assert_eq!(
            Uint256::from(subscriber_1_funds[0].1.amount.u128()),
            Uint256::from(250u128)
        );
        assert_eq!(subscriber_1_funds[0].1.denom, "in_denom".to_string());

        // Creator threshold cancels the stream
        let cancel_msg = StreamSwapExecuteMsg::CancelStreamWithThreshold {};

        let res = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &cancel_msg,
                &[],
            )
            .unwrap();
        let creator_funds = get_funds_from_res(res);
        assert_eq!(creator_funds.len(), 1);
        assert_eq!(
            Uint256::from(creator_funds[0].1.amount.u128()),
            Uint256::from(500u128)
        );
        assert_eq!(creator_funds[0].1.denom, "out_denom".to_string());

        // Creator can not finalize the stream
        let finalize_msg = StreamSwapExecuteMsg::FinalizeStream { new_treasury: None };

        let err = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &finalize_msg,
                &[],
            )
            .unwrap_err();
        let error = err.downcast::<StreamSwapError>().unwrap();
        assert_eq!(error, StreamSwapError::StreamKillswitchActive {});

        // Creator can not cancel the stream again
        let cancel_msg = StreamSwapExecuteMsg::CancelStreamWithThreshold {};

        let err = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &cancel_msg,
                &[],
            )
            .unwrap_err();
        let error = err.downcast::<StreamSwapError>().unwrap();
        assert_eq!(error, StreamSwapError::StreamKillswitchActive {});

        // Subscriber 2 executes exit cancelled after creator cancels stream
        let exit_cancelled_msg = StreamSwapExecuteMsg::ExitCancelled {};

        let res = app
            .execute_contract(
                test_accounts.subscriber_2.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &exit_cancelled_msg,
                &[],
            )
            .unwrap();
        let subscriber_2_funds = get_funds_from_res(res);
        assert_eq!(subscriber_2_funds.len(), 1);
        assert_eq!(
            Uint256::from(subscriber_2_funds[0].1.amount.u128()),
            Uint256::from(1u128)
        );
        assert_eq!(subscriber_2_funds[0].1.denom, "in_denom".to_string());

        // Query stream should return stream with is_cancelled = true
        let query_msg = StreamSwapQueryMsg::Stream {};

        let res: StreamResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &query_msg,
            )
            .unwrap();

        assert_eq!(res.status, Status::Cancelled);
    }

    #[test]
    fn threshold_cancel() {
        let Suite {
            mut app,
            test_accounts,
            stream_swap_code_id,
            stream_swap_factory_code_id,
            vesting_code_id,
        } = SuiteBuilder::default().build();
        let start_time = app.block_info().time.plus_seconds(1_000_000);
        let end_time = app.block_info().time.plus_seconds(5_000_000);
        let bootstrapping_start_time = app.block_info().time.plus_seconds(500_000);
        let threshold = Uint256::from(500u128);

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
            Some("https://sample.url".to_string()),
            test_accounts.creator_1.as_ref(),
            coin(500, "out_denom"),
            "in_denom",
            bootstrapping_start_time,
            start_time,
            end_time,
            Some(threshold),
            None,
            None,
        );

        let res = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                factory_address.clone(),
                &create_stream_msg,
                &[coin(100, "fee_denom"), coin(500, "out_denom")],
            )
            .unwrap();
        let stream_swap_contract_address: String = get_contract_address_from_res(res);
        // Set time to start of the stream
        app.set_block(BlockInfo {
            time: start_time,
            height: 1_000,
            chain_id: "test".to_string(),
        });

        // Subscription 1
        let subscribe_msg = StreamSwapExecuteMsg::Subscribe {};

        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &subscribe_msg,
                &[coin(250, "in_denom")],
            )
            .unwrap();

        // Subscription 2
        let _res = app
            .execute_contract(
                test_accounts.subscriber_2.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &subscribe_msg,
                &[coin(250 - 1, "in_denom")],
            )
            .unwrap();

        // Can not cancel stream before it ends
        let cancel_msg = StreamSwapExecuteMsg::CancelStreamWithThreshold {};

        let err = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &cancel_msg,
                &[],
            )
            .unwrap_err();
        let error = err.downcast::<StreamSwapError>().unwrap();
        assert_eq!(error, StreamSwapError::StreamNotEnded {});

        // Set block to the end of the stream
        app.set_block(BlockInfo {
            time: end_time.plus_seconds(1),
            height: 1_000,
            chain_id: "test".to_string(),
        });

        // Non creator can't cancel stream
        let cancel_msg = StreamSwapExecuteMsg::CancelStreamWithThreshold {};

        let err = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &cancel_msg,
                &[],
            )
            .unwrap_err();

        let error = err.downcast::<StreamSwapError>().unwrap();

        assert_eq!(error, StreamSwapError::Unauthorized {});

        // Creator can cancel stream
        let cancel_msg = StreamSwapExecuteMsg::CancelStreamWithThreshold {};

        let _res = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &cancel_msg,
                &[],
            )
            .unwrap();

        // Query stream should return stream with is_cancelled = true

        let query_msg = StreamSwapQueryMsg::Stream {};

        let res: StreamResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &query_msg,
            )
            .unwrap();

        assert_eq!(res.status, Status::Cancelled);
    }
}
