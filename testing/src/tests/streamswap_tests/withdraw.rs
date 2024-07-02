#[cfg(test)]
mod withdraw_tests {

    use crate::helpers::utils::get_contract_address_from_res;
    #[cfg(test)]
    use crate::helpers::{
        mock_messages::{get_create_stream_msg, get_factory_inst_msg},
        setup::setup,
    };
    use cosmwasm_std::{coin, Addr, BlockInfo, Decimal256, Uint128};
    use cw_multi_test::Executor;
    use streamswap_stream::{
        msg::{
            ExecuteMsg as StreamSwapExecuteMsg, PositionResponse, QueryMsg as StreamSwapQueryMsg,
            StreamResponse,
        },
        ContractError as StreamSwapError,
    };

    #[test]
    fn test_withdraw_pending() {
        let setup_res = setup();
        let test_accounts = setup_res.test_accounts;
        let mut app = setup_res.app;

        // Instantiate stream swap
        let stream_swap_code_id = setup_res.stream_swap_code_id;
        let stream_swap_factory_code_id = setup_res.stream_swap_factory_code_id;
        let vesting_code_id = setup_res.vesting_code_id;
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
                factory_address,
                &create_stream_msg,
                &[coin(100, "fee_denom"), coin(1_000_000, "out_denom")],
            )
            .unwrap();
        let stream_swap_contract_address = get_contract_address_from_res(res);

        // Subscribe to stream
        let subscribe_msg = StreamSwapExecuteMsg::Subscribe {
            operator_target: None,
            operator: None,
        };
        let subscriber_1_balance_before = app
            .wrap()
            .query_balance(test_accounts.subscriber_1.clone(), "in_denom")
            .unwrap();

        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &subscribe_msg,
                &[coin(1_000, "in_denom")],
            )
            .unwrap();
        let subscriber_1_balance_after = app
            .wrap()
            .query_balance(test_accounts.subscriber_1.clone(), "in_denom")
            .unwrap();
        // Subscriber balance should be reduced by 1_000 after subscription
        assert_eq!(
            subscriber_1_balance_before
                .amount
                .checked_sub(Uint128::new(1_000))
                .unwrap(),
            subscriber_1_balance_after.amount
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

        // Query position
        let position: PositionResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapQueryMsg::Position {
                    owner: test_accounts.subscriber_1.clone().into_string(),
                },
            )
            .unwrap();
        assert_eq!(position.purchased, Uint128::zero());
        assert_eq!(position.spent, Uint128::zero());
        assert_eq!(position.shares, Uint128::new(1_000));

        // Withdraw before start time
        let subscriber_1_balance_before = app
            .wrap()
            .query_balance(test_accounts.subscriber_1.clone(), "in_denom")
            .unwrap();

        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapExecuteMsg::Withdraw {
                    cap: Some(Uint128::new(500)),
                    operator_target: None,
                },
                &[],
            )
            .unwrap();
        let subscriber_1_balance_after = app
            .wrap()
            .query_balance(test_accounts.subscriber_1.clone(), "in_denom")
            .unwrap();
        // Subscriber balance should be increased by 1_000 after withdraw
        assert_eq!(
            subscriber_1_balance_before
                .amount
                .checked_add(Uint128::new(500))
                .unwrap(),
            subscriber_1_balance_after.amount
        );
        // Query stream
        let stream: StreamResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapQueryMsg::Stream {},
            )
            .unwrap();
        assert_eq!(stream.dist_index, Decimal256::zero());
        assert_eq!(stream.in_supply, Uint128::new(500));
        assert_eq!(stream.spent_in, Uint128::zero());

        // Withdraw rest of the funds
        let subscriber_1_balance_before = app
            .wrap()
            .query_balance(test_accounts.subscriber_1.clone(), "in_denom")
            .unwrap();
        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapExecuteMsg::Withdraw {
                    cap: None,
                    operator_target: None,
                },
                &[],
            )
            .unwrap();

        let subscriber_1_balance_after = app
            .wrap()
            .query_balance(test_accounts.subscriber_1.clone(), "in_denom")
            .unwrap();
        // Subscriber balance should be increased by 500 after withdraw
        assert_eq!(
            subscriber_1_balance_after
                .amount
                .checked_sub(subscriber_1_balance_before.amount)
                .unwrap(),
            Uint128::new(500)
        );
        // Query stream
        let stream: StreamResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapQueryMsg::Stream {},
            )
            .unwrap();
        assert_eq!(stream.in_supply, Uint128::zero());
        assert_eq!(stream.spent_in, Uint128::new(0));

        // Set block time to end time
        app.set_block(BlockInfo {
            height: 200,
            time: end_time.plus_seconds(1),
            chain_id: "test".to_string(),
        });
        // Exit stream wont work because the subscriber has withdrawn all the funds
        let _err = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapExecuteMsg::ExitStream {
                    operator_target: None,
                    salt: None,
                },
                &[],
            )
            .unwrap_err();
    }

    #[test]
    fn test_withdraw_all_before_exit_case() {
        let setup_res = setup();
        let test_accounts = setup_res.test_accounts;
        let mut app = setup_res.app;

        // Instantiate stream swap
        let stream_swap_code_id = setup_res.stream_swap_code_id;
        let stream_swap_factory_code_id = setup_res.stream_swap_factory_code_id;
        let vesting_code_id = setup_res.vesting_code_id;
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
        let start_time = app.block_info().time.plus_seconds(1000).into();
        let end_time = app.block_info().time.plus_seconds(5000).into();

        let create_stream_msg = get_create_stream_msg(
            &"Stream Swap test".to_string(),
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
                factory_address,
                &create_stream_msg,
                &[coin(100, "fee_denom"), coin(1_000_000_000_000, "out_denom")],
            )
            .unwrap();
        let stream_swap_contract_address = get_contract_address_from_res(res);

        // First subscription
        app.set_block(BlockInfo {
            height: 1000,
            time: start_time,
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
                &[coin(2_000_000_000_000, "in_denom")],
            )
            .unwrap();
        app.set_block(BlockInfo {
            height: 2000,
            time: start_time.plus_seconds(1),
            chain_id: "test".to_string(),
        });

        // Second subscription
        let _res = app
            .execute_contract(
                test_accounts.subscriber_2.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &subscribe_msg,
                &[coin(1_000_000_000_000, "in_denom")],
            )
            .unwrap();

        app.set_block(BlockInfo {
            height: 3000,
            time: start_time.plus_seconds(2),
            chain_id: "test".to_string(),
        });

        // Third subscription
        let _res = app
            .execute_contract(
                test_accounts.admin.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &subscribe_msg,
                &[coin(10, "in_denom")],
            )
            .unwrap();

        // First withdraw
        app.set_block(BlockInfo {
            height: 2000,
            time: start_time.plus_seconds(10),
            chain_id: "test".to_string(),
        });

        let withdraw_msg = StreamSwapExecuteMsg::Withdraw {
            cap: None,
            operator_target: None,
        };
        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &withdraw_msg,
                &[],
            )
            .unwrap();

        // Second withdraw
        let _res = app
            .execute_contract(
                test_accounts.subscriber_2.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &withdraw_msg,
                &[],
            )
            .unwrap();

        // Exit stream
        app.set_block(BlockInfo {
            height: 3000,
            time: end_time.plus_seconds(1),
            chain_id: "test".to_string(),
        });

        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapExecuteMsg::ExitStream {
                    operator_target: None,
                    salt: None,
                },
                &[],
            )
            .unwrap();

        let _res = app
            .execute_contract(
                test_accounts.subscriber_2.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapExecuteMsg::ExitStream {
                    operator_target: None,
                    salt: None,
                },
                &[],
            )
            .unwrap();
    }
    #[test]
    fn test_withdraw() {
        let setup_res = setup();
        let test_accounts = setup_res.test_accounts;
        let mut app = setup_res.app;

        // Instantiate stream swap
        let stream_swap_code_id = setup_res.stream_swap_code_id;
        let stream_swap_factory_code_id = setup_res.stream_swap_factory_code_id;
        let vesting_code_id = setup_res.vesting_code_id;
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
                factory_address,
                &create_stream_msg,
                &[coin(100, "fee_denom"), coin(1_000_000, "out_denom")],
            )
            .unwrap();
        let stream_swap_contract_address = get_contract_address_from_res(res);

        app.set_block(BlockInfo {
            height: 100,
            time: start_time,
            chain_id: "test".to_string(),
        });
        // Subscribe to stream
        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapExecuteMsg::Subscribe {
                    operator_target: None,
                    operator: None,
                },
                &[coin(1_000, "in_denom")],
            )
            .unwrap();

        // Withdraw with cap
        let subscriber_1_balance_before = app
            .wrap()
            .query_balance(test_accounts.subscriber_1.clone(), "in_denom")
            .unwrap();

        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapExecuteMsg::Withdraw {
                    cap: Some(Uint128::new(500)),
                    operator_target: None,
                },
                &[],
            )
            .unwrap();
        let subscriber_1_balance_after = app
            .wrap()
            .query_balance(test_accounts.subscriber_1.clone(), "in_denom")
            .unwrap();
        // Subscriber balance should be increased by 500 after withdraw
        assert_eq!(
            subscriber_1_balance_after
                .amount
                .checked_sub(subscriber_1_balance_before.amount)
                .unwrap(),
            Uint128::new(500)
        );

        // Withdraw amount zero
        let err = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapExecuteMsg::Withdraw {
                    cap: Some(Uint128::zero()),
                    operator_target: None,
                },
                &[],
            )
            .unwrap_err();
        let error = err.source().unwrap();
        let error = error.downcast_ref::<StreamSwapError>().unwrap();
        assert_eq!(error, &StreamSwapError::InvalidWithdrawAmount {});

        // Withdraw amount too high
        let err = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapExecuteMsg::Withdraw {
                    cap: Some(Uint128::new(2_250_000_000_000)),
                    operator_target: None,
                },
                &[],
            )
            .unwrap_err();
        let error = err.source().unwrap();
        let error = error.downcast_ref::<StreamSwapError>().unwrap();
        assert_eq!(
            error,
            &StreamSwapError::WithdrawAmountExceedsBalance(Uint128::new(2_250_000_000_000))
        );

        // Withdraw with valid cap
        let subscriber_1_balance_before = app
            .wrap()
            .query_balance(test_accounts.subscriber_1.clone(), "in_denom")
            .unwrap();

        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapExecuteMsg::Withdraw {
                    cap: Some(Uint128::new(500)),
                    operator_target: None,
                },
                &[],
            )
            .unwrap();
        let subscriber_1_balance_after = app
            .wrap()
            .query_balance(test_accounts.subscriber_1.clone(), "in_denom")
            .unwrap();
        // Subscriber balance should be increased by 500 after withdraw
        assert_eq!(
            subscriber_1_balance_after
                .amount
                .checked_sub(subscriber_1_balance_before.amount)
                .unwrap(),
            Uint128::new(500)
        );
    }
}
