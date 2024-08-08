#[cfg(test)]
mod update_stream {

    use std::str::FromStr;

    use crate::helpers::suite::SuiteBuilder;
    use crate::helpers::{
        mock_messages::{get_create_stream_msg, get_factory_inst_msg},
        suite::Suite,
        utils::get_contract_address_from_res,
    };

    use cosmwasm_std::{coin, Addr, BlockInfo, Decimal256, Uint256};
    use cw_multi_test::Executor;
    use streamswap_types::stream::{
        ExecuteMsg as StreamSwapExecuteMsg, QueryMsg as StreamSwapQueryMsg, Status, StreamResponse,
    };

    #[test]
    fn update_stream_without_subscription() {
        let Suite {
            mut app,
            test_accounts,
            stream_swap_code_id,
            stream_swap_factory_code_id,
            vesting_code_id,
        } = SuiteBuilder::default().build();

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

        let start_time = app.block_info().time.plus_seconds(100);
        let end_time = app.block_info().time.plus_seconds(200);
        let bootstrapping_start_time = app.block_info().time.plus_seconds(50);

        let create_stream_msg = get_create_stream_msg(
            "stream",
            None,
            test_accounts.creator_1.as_ref(),
            coin(100, "out_denom"),
            "in_denom",
            bootstrapping_start_time,
            start_time,
            end_time,
            None,
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
        // Update stream at Waiting status
        let update_stream_msg = StreamSwapExecuteMsg::UpdateStream {};
        let _res = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &update_stream_msg,
                &[],
            )
            .unwrap();
        // Query stream
        let stream: StreamResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &streamswap_types::stream::QueryMsg::Stream {},
            )
            .unwrap();
        assert_eq!(stream.status, Status::Waiting);
        assert_eq!(stream.last_updated, app.block_info().time);

        // 10 seconds later
        app.set_block(BlockInfo {
            height: 1_100,
            time: app.block_info().time.plus_seconds(10),
            chain_id: "test".to_string(),
        });

        // Update stream at Waiting status
        let update_stream_msg = StreamSwapExecuteMsg::UpdateStream {};
        let _res = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &update_stream_msg,
                &[],
            )
            .unwrap();

        // Query stream
        let stream: StreamResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &streamswap_types::stream::QueryMsg::Stream {},
            )
            .unwrap();
        assert_eq!(stream.status, Status::Waiting);
        assert_eq!(stream.last_updated, app.block_info().time);

        // Set time to bootstrapping_start_time+10
        app.set_block(BlockInfo {
            height: 1_100,
            time: bootstrapping_start_time.plus_seconds(10),
            chain_id: "test".to_string(),
        });

        // Update stream at Bootstrapping status
        let update_stream_msg = StreamSwapExecuteMsg::UpdateStream {};
        let _res = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &update_stream_msg,
                &[],
            )
            .unwrap();

        // Query stream
        let stream: StreamResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &streamswap_types::stream::QueryMsg::Stream {},
            )
            .unwrap();
        assert_eq!(stream.status, Status::Bootstrapping);
        assert_eq!(stream.last_updated, app.block_info().time);

        // Set time to start_time+10
        app.set_block(BlockInfo {
            height: 1_100,
            time: start_time.plus_seconds(10),
            chain_id: "test".to_string(),
        });

        // Update stream at Active status
        let update_stream_msg = StreamSwapExecuteMsg::UpdateStream {};
        let _res = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &update_stream_msg,
                &[],
            )
            .unwrap();

        // Query stream
        let stream: StreamResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &streamswap_types::stream::QueryMsg::Stream {},
            )
            .unwrap();
        assert_eq!(stream.status, Status::Active);
        assert_eq!(stream.last_updated, app.block_info().time);
        assert_eq!(stream.dist_index, Decimal256::zero());

        // Now stream is started and 10 seconds passed
        // Subscribe to stream and check
        // Purpose:
        // - We have tried to update stream without subscription at Waiting, Bootstrapping and Active status
        // - Now we will subscribe to stream and update stream at Active status
        // - We will check if stream is updated successfully in next 10 seconds and compare with previous state which no subscription was made
        let subscribe_msg = StreamSwapExecuteMsg::Subscribe {};

        let _res = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &subscribe_msg,
                &[coin(100, "in_denom")],
            )
            .unwrap();

        // Query stream
        let stream: StreamResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &streamswap_types::stream::QueryMsg::Stream {},
            )
            .unwrap();

        assert_eq!(stream.status, Status::Active);
        assert_eq!(stream.last_updated, app.block_info().time);
        assert_eq!(stream.dist_index, Decimal256::zero());
        assert_eq!(stream.in_supply, Uint256::from(100u128));
        assert_eq!(stream.spent_in, Uint256::zero());
        assert_eq!(stream.shares, Uint256::from(100u128));

        // Set time to start_time+20
        app.set_block(BlockInfo {
            height: 1_100,
            time: start_time.plus_seconds(20),
            chain_id: "test".to_string(),
        });

        // Update stream at Active status
        let update_stream_msg = StreamSwapExecuteMsg::UpdateStream {};
        let _res = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &update_stream_msg,
                &[],
            )
            .unwrap();

        // Query stream
        let stream: StreamResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &streamswap_types::stream::QueryMsg::Stream {},
            )
            .unwrap();
        assert_eq!(stream.status, Status::Active);
        assert_eq!(stream.last_updated, app.block_info().time);
        // Calculation:
        // - 10 seconds passed
        // (20-10)/90(time-remaning) = 1/9*100(out_supply) = 11.11/100(shares) = 0.1111
        assert_eq!(stream.dist_index, Decimal256::from_str("0.11").unwrap());
        // Calculation:
        // - 10 seconds passed
        // (20-10)/90(time-remaning) = 1/9*100(in_supply) = 11.11(spent_amount)
        // 100(in_supply) - 11(spent_amount)(round_down) = 89(in_supply)
        assert_eq!(stream.in_supply, Uint256::from(89u128));
        assert_eq!(stream.spent_in, Uint256::from(11u128));
    }

    #[test]
    fn update_stream_during_bootstraping_period() {
        let Suite {
            mut app,
            test_accounts,
            stream_swap_code_id,
            stream_swap_factory_code_id,
            vesting_code_id,
        } = SuiteBuilder::default().build();

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

        let start_time = app.block_info().time.plus_seconds(100);
        let end_time = app.block_info().time.plus_seconds(200);
        let bootstrapping_start_time = app.block_info().time.plus_seconds(50);

        let create_stream_msg = get_create_stream_msg(
            "stream",
            None,
            test_accounts.creator_1.as_ref(),
            coin(100, "out_denom"),
            "in_denom",
            bootstrapping_start_time,
            start_time,
            end_time,
            None,
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

        // Set time to bootstrapping_start_time+10
        app.set_block(BlockInfo {
            height: 1_100,
            time: bootstrapping_start_time.plus_seconds(10),
            chain_id: "test".to_string(),
        });

        let subscribe_msg = StreamSwapExecuteMsg::Subscribe {};

        let _res = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &subscribe_msg,
                &[coin(100, "in_denom")],
            )
            .unwrap();
        // Set time to bootstrapping_start_time+20
        app.set_block(BlockInfo {
            height: 1_100,
            time: bootstrapping_start_time.plus_seconds(20),
            chain_id: "test".to_string(),
        });

        let update_stream_msg = StreamSwapExecuteMsg::UpdateStream {};

        let _res = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &update_stream_msg,
                &[],
            )
            .unwrap();

        let stream: StreamResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &streamswap_types::stream::QueryMsg::Stream {},
            )
            .unwrap();

        assert_eq!(stream.status, Status::Bootstrapping);
        assert_eq!(stream.in_supply, Uint256::from(100u128));
        assert_eq!(stream.dist_index, Decimal256::zero());
        assert_eq!(stream.spent_in, Uint256::zero());

        app.set_block(BlockInfo {
            height: 1_100,
            time: start_time.plus_seconds(10),
            chain_id: "test".to_string(),
        });

        let _res = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &update_stream_msg,
                &[],
            )
            .unwrap();

        let stream: StreamResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &streamswap_types::stream::QueryMsg::Stream {},
            )
            .unwrap();

        assert_eq!(stream.in_supply + stream.spent_in, Uint256::from(100u128));
        assert_eq!(stream.out_remaining, Uint256::from(90u128));
        assert_ne!(stream.dist_index, Decimal256::zero());
        assert_ne!(stream.spent_in, Uint256::zero());
    }

    #[test]
    fn price_feed() {
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

        let create_stream_msg = get_create_stream_msg(
            "Stream Swap tests",
            None,
            test_accounts.creator_1.as_ref(),
            coin(1_000, "out_denom"),
            "in_denom",
            bootstrapping_start_time,
            start_time,
            end_time,
            None,
            None,
            None,
        );
        // Create Stream
        let res = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                factory_address.clone(),
                &create_stream_msg,
                &[coin(100, "fee_denom"), coin(1_000, "out_denom")],
            )
            .unwrap();

        let stream_swap_contract_address: String = get_contract_address_from_res(res);

        app.set_block(BlockInfo {
            height: 1_100,
            time: start_time,
            chain_id: "test".to_string(),
        });

        // Check current streamed price before update
        let stream: StreamResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapQueryMsg::Stream {},
            )
            .unwrap();
        assert_eq!(
            stream.current_streamed_price,
            Decimal256::new(Uint256::zero())
        );

        let subscribe_msg = StreamSwapExecuteMsg::Subscribe {};
        // First subscription
        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &subscribe_msg,
                &[coin(1_000, "in_denom")],
            )
            .unwrap();

        // Update environment to start_time plus 50 sec
        app.set_block(BlockInfo {
            height: 1_100,
            time: start_time.plus_seconds(50),
            chain_id: "test".to_string(),
        });
        // Update Stream
        let update_stream_msg = StreamSwapExecuteMsg::UpdateStream {};
        let _res = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &update_stream_msg,
                &[coin(100, "fee_denom")],
            )
            .unwrap();

        // Check current streamed price after update
        let stream: StreamResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapQueryMsg::Stream {},
            )
            .unwrap();
        assert_eq!(
            stream.current_streamed_price,
            Decimal256::from_str("1").unwrap()
        );

        // Second subscription
        let _res = app
            .execute_contract(
                test_accounts.subscriber_2.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &subscribe_msg,
                &[coin(1_000, "in_denom")],
            )
            .unwrap();

        // Set time to start_time plus 75 secs
        app.set_block(BlockInfo {
            height: 1_100,
            time: start_time.plus_seconds(75),
            chain_id: "test".to_string(),
        });
        // Update Stream
        let update_stream_msg = StreamSwapExecuteMsg::UpdateStream {};
        let _res = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &update_stream_msg,
                &[coin(100, "fee_denom")],
            )
            .unwrap();

        let stream: StreamResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapQueryMsg::Stream {},
            )
            .unwrap();

        assert_eq!(
            stream.current_streamed_price,
            Decimal256::from_str("3").unwrap()
        );
    }
}
