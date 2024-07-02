#[cfg(test)]
mod treshold_tests {

    use crate::helpers::utils::{get_contract_address_from_res, get_funds_from_res};
    use crate::helpers::{
        mock_messages::{get_create_stream_msg, get_factory_inst_msg},
        setup::{setup, SetupResponse},
    };
    use cosmwasm_std::{coin, Addr, BlockInfo, Uint128};
    use cw_multi_test::Executor;
    use streamswap_stream::state::Status;
    use streamswap_stream::threshold;
    use streamswap_stream::{
        msg::{ExecuteMsg as StreamSwapExecuteMsg, QueryMsg as StreamSwapQueryMsg, StreamResponse},
        ContractError as StreamSwapError,
    };

    #[test]
    fn test_threshold_reached() {
        let SetupResponse {
            mut app,
            test_accounts,
            stream_swap_code_id,
            stream_swap_factory_code_id,
            vesting_code_id,
        } = setup();
        let start_time = app.block_info().time.plus_seconds(1_000_000).into();
        let end_time = app.block_info().time.plus_seconds(5_000_000).into();
        let threshold = Uint128::from(250u128);

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
            coin(500, "out_denom"),
            "in_denom",
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

        let subscribe_msg = StreamSwapExecuteMsg::Subscribe {
            operator_target: None,
            operator: None,
        };

        let res = app
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
        let exit_msg = StreamSwapExecuteMsg::ExitStream {
            operator_target: None,
            salt: None,
        };

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
        assert_eq!(funds[0].1.amount, Uint128::from(500u128));
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
        assert_eq!(funds[0].1.amount, Uint128::from(495u128));
        assert_eq!(funds[0].1.denom, "in_denom".to_string());
        // Fee collector's revenue
        assert_eq!(funds[1].1.amount, Uint128::from(5u128));
        assert_eq!(funds[1].1.denom, "in_denom".to_string());
    }

    #[test]
    fn test_threshold_not_reached() {
        let SetupResponse {
            mut app,
            test_accounts,
            stream_swap_code_id,
            stream_swap_factory_code_id,
            vesting_code_id,
        } = setup();
        let start_time = app.block_info().time.plus_seconds(1_000_000).into();
        let end_time = app.block_info().time.plus_seconds(5_000_000).into();
        let threshold = Uint128::from(500u128);

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
            coin(500, "out_denom"),
            "in_denom",
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

        let subscribe_msg = StreamSwapExecuteMsg::Subscribe {
            operator_target: None,
            operator: None,
        };
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
        let exit_msg = StreamSwapExecuteMsg::ExitStream {
            operator_target: None,
            salt: None,
        };

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
            StreamSwapError::ThresholdError(threshold::ThresholdError::ThresholdNotReached {})
        );

        // Subscriber one executes exit cancelled before creator cancels stream
        let exit_cancelled_msg = StreamSwapExecuteMsg::ExitCancelled {
            operator_target: None,
        };
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
        assert_eq!(subscriber_1_funds[0].1.amount, Uint128::from(250u128));
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
        assert_eq!(creator_funds[0].1.amount, Uint128::from(500u128));
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
        let exit_cancelled_msg = StreamSwapExecuteMsg::ExitCancelled {
            operator_target: None,
        };

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
        assert_eq!(subscriber_2_funds[0].1.amount, Uint128::from(1u128));
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
    fn test_threshold_cancel() {
        let SetupResponse {
            mut app,
            test_accounts,
            stream_swap_code_id,
            stream_swap_factory_code_id,
            vesting_code_id,
        } = setup();
        let start_time = app.block_info().time.plus_seconds(1_000_000).into();
        let end_time = app.block_info().time.plus_seconds(5_000_000).into();
        let threshold = Uint128::from(500u128);

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
            coin(500, "out_denom"),
            "in_denom",
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

        // Subscription 1
        let subscribe_msg = StreamSwapExecuteMsg::Subscribe {
            operator_target: None,
            operator: None,
        };

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

        let res = app
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
//mod threshold {
//         use crate::{
//             killswitch::{execute_cancel_stream_with_threshold, execute_exit_cancelled},
//             threshold::ThresholdError,
//         };

//         // Create a stream with a threshold
//         // Subscribe to the stream
//         use super::*;

//         #[test]
//         fn test_threshold_reached() {
//             let treasury = Addr::unchecked("treasury");
//             let start = Timestamp::from_seconds(1_000_000);
//             let end = Timestamp::from_seconds(5_000_000);
//             let out_supply = Uint128::new(500);
//             let out_denom = "out_denom";
//             let in_denom = "in_denom";

//             // threshold = 500*0.5 / 1-0.01 =252.5

//             // instantiate
//             let mut deps = mock_dependencies();
//             let mut env = mock_env();
//             env.block.time = Timestamp::from_seconds(0);
//             let msg = crate::msg::InstantiateMsg {
//                 min_stream_seconds: Uint64::new(1000),
//                 min_seconds_until_start_time: Uint64::new(0),
//                 stream_creation_denom: "fee".to_string(),
//                 stream_creation_fee: Uint128::new(100),
//                 exit_fee_percent: Decimal::percent(1),
//                 fee_collector: "collector".to_string(),
//                 protocol_admin: "protocol_admin".to_string(),
//                 accepted_in_denom: in_denom.to_string(),
//             };
//             instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

//             // create stream
//             let mut env = mock_env();
//             env.block.time = Timestamp::from_seconds(0);
//             let info = mock_info(
//                 "creator",
//                 &[
//                     Coin::new(out_supply.u128(), out_denom),
//                     Coin::new(100, "fee"),
//                 ],
//             );
//             execute_create_stream(
//                 deps.as_mut(),
//                 env,
//                 info,
//                 treasury.to_string(),
//                 "test".to_string(),
//                 Some("https://sample.url".to_string()),
//                 in_denom.to_string(),
//                 out_denom.to_string(),
//                 out_supply,
//                 start,
//                 end,
//                 Some(Uint128::from(250u128)),
//             )
//             .unwrap();

//             // subscription
//             let mut env = mock_env();
//             env.block.time = start;
//             let funds = Coin::new(252, "in_denom");
//             let info = mock_info("subscriber", &[funds]);
//             let msg = crate::msg::ExecuteMsg::Subscribe {
//                 stream_id: 1,
//                 operator_target: None,
//                 operator: Some("operator".to_string()),
//             };
//             let _res = execute(deps.as_mut(), env, info, msg).unwrap();

//             // Threshold should be reached
//             let mut env = mock_env();
//             env.block.time = end.plus_seconds(1);

//             // Exit should be possible
//             // Since there is only one subscriber all out denom should be sent to subscriber
//             // In calculations we are always rounding down that one token will be left in the stream
//             // Asuming token is 6 decimals
//             // This amount could be considered as insignificant
//             let info = mock_info("subscriber", &[]);
//             let res = execute_exit_stream(deps.as_mut(), env.clone(), info, 1, None).unwrap();
//             assert_eq!(
//                 res.messages,
//                 vec![SubMsg::new(BankMsg::Send {
//                     to_address: "subscriber".to_string(),
//                     amount: vec![Coin::new(499, "out_denom")],
//                 })],
//             );

//             // Creator finalizes the stream
//             let info = mock_info("treasury", &[]);
//             let res = execute_finalize_stream(deps.as_mut(), env.clone(), info, 1, None).unwrap();
//             // Creator's revenue
//             assert_eq!(
//                 res.messages[0].msg,
//                 cosmwasm_std::CosmosMsg::Bank(BankMsg::Send {
//                     to_address: "treasury".to_string(),
//                     amount: vec![Coin::new(250, "in_denom")],
//                 })
//             );
//             assert_eq!(
//                 res.messages[1].msg,
//                 cosmwasm_std::CosmosMsg::Bank(BankMsg::Send {
//                     to_address: "collector".to_string(),
//                     amount: vec![Coin::new(100, "fee")],
//                 })
//             );
//             assert_eq!(
//                 res.messages[2].msg,
//                 cosmwasm_std::CosmosMsg::Bank(BankMsg::Send {
//                     to_address: "collector".to_string(),
//                     amount: vec![Coin::new(2, "in_denom")],
//                 })
//             )
//         }

//         #[test]
//         fn test_threshold_not_reached() {
//             let treasury = Addr::unchecked("treasury");
//             let start = Timestamp::from_seconds(1_000_000);
//             let end = Timestamp::from_seconds(5_000_000);
//             let out_supply = Uint128::new(500);
//             let out_denom = "out_denom";
//             let in_denom = "in_denom";

//             // threshold = 500*0.5 / 1-0.01 =252.5

//             // instantiate
//             let mut deps = mock_dependencies();
//             let mut env = mock_env();
//             env.block.height = 0;
//             let msg = crate::msg::InstantiateMsg {
//                 min_stream_seconds: Uint64::new(1000),
//                 min_seconds_until_start_time: Uint64::new(0),
//                 stream_creation_denom: "fee".to_string(),
//                 stream_creation_fee: Uint128::new(100),
//                 exit_fee_percent: Decimal::percent(1),
//                 fee_collector: "collector".to_string(),
//                 protocol_admin: "protocol_admin".to_string(),
//                 accepted_in_denom: in_denom.to_string(),
//             };
//             instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

//             // create stream
//             let mut env = mock_env();
//             env.block.time = Timestamp::from_seconds(0);
//             let info = mock_info(
//                 "creator",
//                 &[
//                     Coin::new(out_supply.u128(), out_denom),
//                     Coin::new(100, "fee"),
//                 ],
//             );
//             execute_create_stream(
//                 deps.as_mut(),
//                 env,
//                 info,
//                 treasury.to_string(),
//                 "test".to_string(),
//                 Some("https://sample.url".to_string()),
//                 in_denom.to_string(),
//                 out_denom.to_string(),
//                 out_supply,
//                 start,
//                 end,
//                 Some(500u128.into()),
//             )
//             .unwrap();

//             // Subscription 1
//             let mut env = mock_env();
//             env.block.time = start;
//             let funds = Coin::new(250, "in_denom");
//             let info = mock_info("subscriber", &[funds]);
//             let msg = crate::msg::ExecuteMsg::Subscribe {
//                 stream_id: 1,
//                 operator_target: None,
//                 operator: Some("operator".to_string()),
//             };
//             let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

//             // Subscription 2
//             let funds = Coin::new(1, "in_denom");
//             let info = mock_info("subscriber2", &[funds]);
//             let msg = crate::msg::ExecuteMsg::Subscribe {
//                 stream_id: 1,
//                 operator_target: None,
//                 operator: Some("operator".to_string()),
//             };
//             let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

//             // Set time to the end of the stream
//             let mut env = mock_env();
//             env.block.time = end.plus_seconds(1);

//             // Exit should not be possible
//             let info = mock_info("subscriber", &[]);
//             let res = execute_exit_stream(deps.as_mut(), env.clone(), info, 1, None).unwrap_err();
//             assert_eq!(
//                 res,
//                 ContractError::ThresholdError(ThresholdError::ThresholdNotReached {})
//             );

//             // Finalize should not be possible
//             let info = mock_info("treasury", &[]);
//             let res =
//                 execute_finalize_stream(deps.as_mut(), env.clone(), info, 1, None).unwrap_err();
//             assert_eq!(
//                 res,
//                 ContractError::ThresholdError(ThresholdError::ThresholdNotReached {})
//             );

//             // Subscriber one executes exit cancelled before creator cancels stream
//             let info = mock_info("subscriber", &[]);
//             let res = execute_exit_cancelled(deps.as_mut(), env.clone(), info, 1, None).unwrap();
//             assert_eq!(
//                 res.messages,
//                 vec![SubMsg::new(BankMsg::Send {
//                     to_address: "subscriber".to_string(),
//                     amount: vec![Coin::new(250, "in_denom")],
//                 })]
//             );
//             // Creator threshold cancels the stream
//             let info = mock_info("treasury", &[]);
//             let res =
//                 execute_cancel_stream_with_threshold(deps.as_mut(), env.clone(), info, 1).unwrap();
//             assert_eq!(
//                 res.messages,
//                 vec![
//                     // Out denom refunded
//                     SubMsg::new(BankMsg::Send {
//                         to_address: "treasury".to_string(),
//                         amount: vec![Coin::new(500, "out_denom")],
//                     }),
//                 ]
//             );
//             // Creator can not finalize the stream
//             let info = mock_info("treasury", &[]);
//             let res =
//                 execute_finalize_stream(deps.as_mut(), env.clone(), info, 1, None).unwrap_err();
//             assert_eq!(res, ContractError::StreamKillswitchActive {});

//             // Creator can not cancel the stream again
//             let info = mock_info("treasury", &[]);
//             let res = execute_cancel_stream_with_threshold(deps.as_mut(), env.clone(), info, 1)
//                 .unwrap_err();
//             assert_eq!(res, ContractError::StreamKillswitchActive {});

//             // Subscriber 2 executes exit cancelled after creator cancels stream
//             let info = mock_info("subscriber2", &[]);
//             let res = execute_exit_cancelled(deps.as_mut(), env.clone(), info, 1, None).unwrap();
//             assert_eq!(
//                 // In denom refunded
//                 res.messages,
//                 vec![SubMsg::new(BankMsg::Send {
//                     to_address: "subscriber2".to_string(),
//                     amount: vec![Coin::new(1, "in_denom")],
//                 })]
//             );
//         }

//         #[test]
//         fn test_threshold_cancel() {
//             let treasury = Addr::unchecked("treasury");
//             let start = Timestamp::from_seconds(1_000_000);
//             let end = Timestamp::from_seconds(5_000_000);
//             let out_supply = Uint128::new(500);
//             let out_denom = "out_denom";
//             let in_denom = "in_denom";

//             // threshold = 500*0.5 / 1-0.01 =252.5

//             // instantiate
//             let mut deps = mock_dependencies();
//             let mut env = mock_env();
//             env.block.time = Timestamp::from_seconds(0);
//             let msg = crate::msg::InstantiateMsg {
//                 min_stream_seconds: Uint64::new(1000),
//                 min_seconds_until_start_time: Uint64::new(0),
//                 stream_creation_denom: "fee".to_string(),
//                 stream_creation_fee: Uint128::new(100),
//                 exit_fee_percent: Decimal::percent(1),
//                 fee_collector: "collector".to_string(),
//                 protocol_admin: "protocol_admin".to_string(),
//                 accepted_in_denom: in_denom.to_string(),
//             };
//             instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

//             // create stream
//             let mut env = mock_env();
//             env.block.time = Timestamp::from_seconds(0);
//             let info = mock_info(
//                 "creator",
//                 &[
//                     Coin::new(out_supply.u128(), out_denom),
//                     Coin::new(100, "fee"),
//                 ],
//             );
//             execute_create_stream(
//                 deps.as_mut(),
//                 env,
//                 info,
//                 treasury.to_string(),
//                 "test".to_string(),
//                 Some("https://sample.url".to_string()),
//                 in_denom.to_string(),
//                 out_denom.to_string(),
//                 out_supply,
//                 start,
//                 end,
//                 Some(1_000u128.into()),
//             )
//             .unwrap();

//             // Subscription 1
//             let mut env = mock_env();
//             env.block.time = start;
//             let funds = Coin::new(250, "in_denom");
//             let info = mock_info("subscriber", &[funds]);
//             let msg = crate::msg::ExecuteMsg::Subscribe {
//                 stream_id: 1,
//                 operator_target: None,
//                 operator: Some("operator".to_string()),
//             };
//             let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

//             // Subscription 2
//             let funds = Coin::new(500, "in_denom");
//             let info = mock_info("subscriber2", &[funds]);
//             let msg = crate::msg::ExecuteMsg::Subscribe {
//                 stream_id: 1,
//                 operator_target: None,
//                 operator: Some("operator".to_string()),
//             };
//             let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
//             // Can not cancel stream before it ends
//             let mut env = mock_env();
//             env.block.time = start.plus_seconds(1_000_000);
//             let res = execute_cancel_stream_with_threshold(
//                 deps.as_mut(),
//                 env,
//                 mock_info("treasury", &[]),
//                 1,
//             )
//             .unwrap_err();
//             assert_eq!(res, ContractError::StreamNotEnded {});

//             // Set block to the end of the stream
//             let mut env = mock_env();
//             env.block.time = end.plus_seconds(1);

//             // Non creator can't cancel stream
//             let res = execute_cancel_stream_with_threshold(
//                 deps.as_mut(),
//                 env.clone(),
//                 mock_info("random", &[]),
//                 1,
//             )
//             .unwrap_err();
//             assert_eq!(res, ContractError::Unauthorized {});

//             // Creator can cancel stream
//             let _res = execute_cancel_stream_with_threshold(
//                 deps.as_mut(),
//                 env.clone(),
//                 mock_info("treasury", &[]),
//                 1,
//             )
//             .unwrap();
//             // Query stream should return stream with is_cancelled = true
//             let stream = query_stream(deps.as_ref(), env.clone(), 1).unwrap();
//             assert_eq!(stream.status, Status::Cancelled);
//         }
//     }
// }
