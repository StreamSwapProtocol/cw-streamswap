#[cfg(test)]
mod pool_tests {
    use crate::helpers::mock_messages::{get_create_stream_msg, get_factory_inst_msg};
    use crate::helpers::suite::{Suite, SuiteBuilder};
    use crate::helpers::utils::get_contract_address_from_res;
    use cosmwasm_std::{coin, Addr, Coin};
    use cw_multi_test::Executor;
    use osmosis_std::types::osmosis::concentratedliquidity::poolmodel::concentrated::v1beta1::MsgCreateConcentratedPool;
    use streamswap_types::factory::CreatePool;
    use streamswap_types::stream::ExecuteMsg;

    #[test]
    fn pool_creation() {
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

        let in_denom = "in_denom";
        let _out_supply = 1_000_000_000_000u128;
        let out_denom = "out_denom";
        // %20 of out_supply will go to pool
        let out_clp_amount = 200_000_000_000u128;
        // this is mocked by querier at test_helpers.rs
        let _pool_creation_fee = 1000000;
        let _pool_creation_denom = "fee_denom";
        let stream_creation_denom = "fee_denom";
        let _stream_creation_fee = 100;
        let subs1_token = Coin::new(1_000_000_000, in_denom);
        let subs2_token = Coin::new(3_000_000_000, in_denom);

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
            None,
            test_accounts.creator_1.as_ref(),
            coin(1_000_000, out_denom),
            in_denom,
            bootstrapping_start_time,
            start_time,
            end_time,
            None,
            Some(CreatePool {
                out_amount_clp: out_clp_amount.into(),
                msg_create_pool: MsgCreateConcentratedPool {
                    sender: test_accounts.creator_1.to_string(),
                    denom0: in_denom.to_string(),
                    denom1: out_denom.to_string(),
                    tick_spacing: 100,
                    spread_factor: "10".to_string(),
                },
            }),
            None,
        );
        let res = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                factory_address.clone(),
                &create_stream_msg,
                &[coin(100, stream_creation_denom), coin(1_000_000, out_denom)],
            )
            .unwrap();
        let stream_swap_contract_address: String = get_contract_address_from_res(res);

        // First Subscription
        let subscribe_msg = ExecuteMsg::Subscribe {};
        app.update_block(|b| b.time = start_time.plus_seconds(100));
        app.execute_contract(
            test_accounts.subscriber_1.clone(),
            Addr::unchecked(stream_swap_contract_address.clone()),
            &subscribe_msg,
            &[subs1_token],
        )
        .unwrap();

        // Second Subscription
        app.update_block(|b| b.time = start_time.plus_seconds(100_000));
        app.execute_contract(
            test_accounts.subscriber_2.clone(),
            Addr::unchecked(stream_swap_contract_address.clone()),
            &subscribe_msg,
            &[subs2_token],
        )
        .unwrap();
        // finalize stream
        // check outgoing messages
        app.update_block(|b| {
            b.time = end_time.plus_seconds(100_000);
        });
        app.execute_contract(
            test_accounts.creator_1.clone(),
            Addr::unchecked(stream_swap_contract_address.clone()),
            &ExecuteMsg::FinalizeStream { new_treasury: None },
            &[],
        )
        .unwrap();
    }
}
