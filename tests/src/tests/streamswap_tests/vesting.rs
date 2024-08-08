#[cfg(test)]
mod vesting {
    use crate::helpers::mock_messages::{get_create_stream_msg, get_factory_inst_msg};
    use crate::helpers::suite::{Suite, SuiteBuilder};
    use crate::helpers::utils::{
        get_contract_address_from_res, get_funds_from_res, get_wasm_attribute_with_key,
    };
    use cosmwasm_std::{coin, Addr, Binary, Coin, Uint128};
    use cw_multi_test::Executor;
    use cw_vesting::msg::InstantiateMsg as VestingInstantiateMsg;
    use cw_vesting::vesting::Schedule;
    use cw_vesting::UncheckedDenom;
    use streamswap_types::stream::{
        ExecuteMsg as StreamSwapExecuteMsg, QueryMsg as StreamSwapQueryMsg, Status, StreamResponse,
    };

    #[test]
    fn vesting() {
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

        let vesting_msg = VestingInstantiateMsg {
            owner: None,
            recipient: test_accounts.subscriber_1.to_string(),
            title: "Streamswap vesting".to_string(),
            description: None,
            total: Uint128::new(0),
            denom: UncheckedDenom::Native("out_denom".to_string()),
            schedule: Schedule::SaturatingLinear,
            start_time: None,
            vesting_duration_seconds: 150,
            unbonding_duration_seconds: 0,
        };
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
            Some(vesting_msg),
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
        let subscribe_msg = StreamSwapExecuteMsg::Subscribe {};
        app.update_block(|b| b.time = start_time);
        // First subscription
        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &subscribe_msg,
                &[coin(200, "in_denom")],
            )
            .unwrap();

        // update block time
        app.update_block(|b| b.time = end_time.plus_seconds(5));

        let finalized_msg = StreamSwapExecuteMsg::FinalizeStream { new_treasury: None };
        let res = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &finalized_msg,
                &[],
            )
            .unwrap();

        let stream_swap_funds = get_funds_from_res(res);
        assert_eq!(
            stream_swap_funds,
            vec![
                (
                    String::from(test_accounts.creator_1.clone()),
                    Coin {
                        denom: "in_denom".to_string(),
                        amount: Uint128::new(198)
                    }
                ),
                (
                    String::from(test_accounts.admin.clone(),),
                    Coin {
                        denom: "in_denom".to_string(),
                        amount: Uint128::new(2)
                    }
                ),
            ]
        );
        // Query the stream status
        let stream: StreamResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapQueryMsg::Stream {},
            )
            .unwrap();

        assert_eq!(stream.status, Status::Finalized);

        // no salt expect error
        let exit_msg = StreamSwapExecuteMsg::ExitStream { salt: None };
        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &exit_msg,
                &[],
            )
            .unwrap_err();

        // sub1 exists
        let exit_msg = StreamSwapExecuteMsg::ExitStream {
            salt: Some(Binary::from_base64("salt").unwrap()),
        };
        let res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &exit_msg,
                &[],
            )
            .unwrap();

        let vesting_addr = get_wasm_attribute_with_key(res, "vesting_address".to_string());
        let contract_data = app.contract_data(&Addr::unchecked(vesting_addr)).unwrap();

        // Not the best test :(
        assert_eq!(contract_data.code_id, vesting_code_id);
    }
}
