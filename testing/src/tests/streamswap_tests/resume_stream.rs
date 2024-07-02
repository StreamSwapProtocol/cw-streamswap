#[cfg(test)]
mod resume_stream_test {

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
    fn test_resume() {
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
        // create stream
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
        // subscription
        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &subscribe_msg,
                &[coin(150, "in_denom")],
            )
            .unwrap();

        // can't resume if not paused
        let resume_stream_msg = StreamSwapExecuteMsg::ResumeStream {};
        let err = app
            .execute_contract(
                test_accounts.admin.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &resume_stream_msg,
                &[],
            )
            .unwrap_err();
        let error = err.source().unwrap();
        let error = error.downcast_ref::<StreamSwapError>().unwrap();
        assert_eq!(*error, StreamSwapError::StreamNotPaused {});

        // pause stream
        let pause_stream_msg = StreamSwapExecuteMsg::PauseStream {};
        let pause_date = start_time.plus_seconds(20);
        app.set_block(BlockInfo {
            height: 1_100,
            time: pause_date,
            chain_id: "test".to_string(),
        });
        let _res = app
            .execute_contract(
                test_accounts.admin.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &pause_stream_msg,
                &[],
            )
            .unwrap();

        // resume stream
        let resume_date = start_time.plus_seconds(30);
        app.set_block(BlockInfo {
            height: 1_100,
            time: resume_date,
            chain_id: "test".to_string(),
        });
        let _res = app
            .execute_contract(
                test_accounts.admin.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &resume_stream_msg,
                &[],
            )
            .unwrap();

        // new end date is correct
        let new_end_date = end_time.plus_nanos(resume_date.nanos() - pause_date.nanos());
        let stream: StreamResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapQueryMsg::Stream {},
            )
            .unwrap();
        assert_eq!(stream.end_time, new_end_date);
    }
}
