#[cfg(test)]
mod withdraw_paused_test {

    use std::str::FromStr;

    use crate::helpers::utils::{get_contract_address_from_res, get_funds_from_res};
    use crate::helpers::{
        mock_messages::{get_create_stream_msg, get_factory_inst_msg},
        setup::{setup, SetupResponse},
    };
    use cosmwasm_std::{coin, Addr, BlockInfo, Coin, Decimal256, Uint128};
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
    fn test_withdraw_pause() {
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
        // create Stream
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
            operator: Some(test_accounts.subscriber_2.to_string()),
        };
        // subscription
        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &subscribe_msg,
                &[coin(150, "in_denom")],
            )
            .unwrap();

        // withdraw with cap
        app.set_block(BlockInfo {
            height: 1_100,
            time: start_time.plus_seconds(10),
            chain_id: "test".to_string(),
        });
        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapExecuteMsg::Withdraw {
                    cap: Some(Uint128::new(100)),
                    operator_target: None,
                },
                &[],
            )
            .unwrap();
        let position: PositionResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapQueryMsg::Position {
                    owner: test_accounts.subscriber_1.clone().into_string(),
                },
            )
            .unwrap();
        assert_eq!(position.purchased, Uint128::new(99));
        assert_eq!(position.spent, Uint128::new(15));
        assert_eq!(position.in_balance, Uint128::new(35));
        // first fund amount should be equal to in_balance + spent + cap
        assert_eq!(
            position.in_balance + position.spent + Uint128::new(100),
            Uint128::new(150)
        );

        // cant execute WithdrawPaused before its paused
        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapExecuteMsg::WithdrawPaused {
                    cap: (Some(Uint128::new(100))),
                    operator_target: (None),
                },
                &[],
            )
            .unwrap_err();

        let err = _res.source().unwrap();
        let error = err.downcast_ref::<StreamSwapError>().unwrap();
        assert_eq!(*error, StreamSwapError::StreamNotPaused {});

        // pause stream
        let pause_stream_msg = StreamSwapExecuteMsg::PauseStream {};
        let _res = app
            .execute_contract(
                test_accounts.admin.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &pause_stream_msg,
                &[],
            )
            .unwrap();

        let stream1_old: StreamResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapQueryMsg::Stream {},
            )
            .unwrap();

        // unauthorized check
        let _res = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapExecuteMsg::WithdrawPaused {
                    cap: (Some(Uint128::new(10))),
                    operator_target: (Some((test_accounts.subscriber_1.to_string()))),
                },
                &[],
            )
            .unwrap_err();
        let err = _res.source().unwrap();
        let error = err.downcast_ref::<StreamSwapError>().unwrap();
        assert_eq!(*error, StreamSwapError::Unauthorized {});

        // cap exceeds in balance check
        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapExecuteMsg::WithdrawPaused {
                    cap: (Some(Uint128::new(150))),
                    operator_target: (None),
                },
                &[],
            )
            .unwrap_err();
        let err = _res.source().unwrap();
        let error = err.downcast_ref::<StreamSwapError>().unwrap();
        assert_eq!(
            *error,
            StreamSwapError::WithdrawAmountExceedsBalance(Uint128::new(150))
        );

        // withdraw cap is zero
        let err = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapExecuteMsg::WithdrawPaused {
                    cap: (Some(Uint128::zero())),
                    operator_target: (None),
                },
                &[],
            )
            .unwrap_err();
        let error = err.source().unwrap();
        let error = error.downcast_ref::<StreamSwapError>().unwrap();
        assert_eq!(error, &StreamSwapError::InvalidWithdrawAmount {});

        // withdraw with cap
        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapExecuteMsg::WithdrawPaused {
                    cap: (Some(Uint128::new(10))),
                    operator_target: (None),
                },
                &[],
            )
            .unwrap();

        // withdraw after pause
        let res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapExecuteMsg::WithdrawPaused {
                    cap: (None),
                    operator_target: (None),
                },
                &[],
            )
            .unwrap();
        let stream_swap_funds = get_funds_from_res(res);
        assert_eq!(
            stream_swap_funds,
            vec![(
                String::from(test_accounts.subscriber_1.clone()),
                Coin {
                    denom: "in_denom".to_string(),
                    amount: Uint128::new(25)
                }
            ),]
        );

        let stream1_new: StreamResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapQueryMsg::Stream {},
            )
            .unwrap();
        assert_eq!(stream1_old.dist_index, stream1_new.dist_index);
        assert_eq!(stream1_new.in_supply, Uint128::zero());
        assert_eq!(stream1_new.shares, Uint128::zero());

        let position: PositionResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapQueryMsg::Position {
                    owner: test_accounts.subscriber_1.clone().into_string(),
                },
            )
            .unwrap();
        assert_eq!(position.in_balance, Uint128::zero());
        assert_eq!(position.spent, Uint128::new(15));
        assert_eq!(position.purchased, Uint128::new(99));
        assert_eq!(position.shares, Uint128::zero());
    }
}
