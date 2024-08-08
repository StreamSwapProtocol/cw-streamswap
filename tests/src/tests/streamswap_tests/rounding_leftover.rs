#[cfg(test)]
mod rounding_leftover {

    use std::str::FromStr;

    use crate::helpers::suite::SuiteBuilder;
    use crate::helpers::utils::get_contract_address_from_res;
    use crate::helpers::{
        mock_messages::{get_create_stream_msg, get_factory_inst_msg},
        suite::Suite,
    };
    use cosmwasm_std::Uint256;
    use cosmwasm_std::{coin, Addr, BlockInfo, Decimal256, Timestamp};
    use cw_multi_test::Executor;
    use streamswap_types::stream::{
        ExecuteMsg as StreamSwapExecuteMsg, QueryMsg as StreamSwapQueryMsg,
    };
    use streamswap_types::stream::{PositionResponse, StreamResponse};

    #[test]
    fn rounding_leftover() {
        let Suite {
            mut app,
            test_accounts,
            stream_swap_code_id,
            stream_swap_factory_code_id,
            vesting_code_id,
        } = SuiteBuilder::default().build();

        let start_time = Timestamp::from_seconds(1_000_000);
        let end_time = Timestamp::from_seconds(5_000_000);
        let bootstrapping_start_time = Timestamp::from_seconds(500_000);

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
            coin(1_000_000_000_000, "out_denom"),
            "in_denom",
            bootstrapping_start_time,
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
                &[coin(100, "fee_denom"), coin(1_000_000_000_000, "out_denom")],
            )
            .unwrap();
        let stream_swap_contract_address: String = get_contract_address_from_res(res);

        app.set_block(BlockInfo {
            time: start_time.plus_seconds(100),
            height: 1,
            chain_id: "SS".to_string(),
        });
        // First subscription
        let subscribe_msg = StreamSwapExecuteMsg::Subscribe {};
        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &subscribe_msg,
                &[coin(1_000_000_000, "in_denom")],
            )
            .unwrap();

        // Second subscription
        app.set_block(BlockInfo {
            time: start_time.plus_seconds(100_000),
            height: 2,
            chain_id: "SS".to_string(),
        });
        let _res = app
            .execute_contract(
                test_accounts.subscriber_2.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &subscribe_msg,
                &[coin(3_000_000_000, "in_denom")],
            )
            .unwrap();

        // Update position of subscriber 1
        app.set_block(BlockInfo {
            time: start_time.plus_seconds(3_000_000),
            height: 3,
            chain_id: "SS".to_string(),
        });
        let update_position_msg = StreamSwapExecuteMsg::UpdatePosition {};
        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &update_position_msg,
                &[],
            )
            .unwrap();

        let position_1: PositionResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapQueryMsg::Position {
                    owner: test_accounts.subscriber_1.to_string(),
                },
            )
            .unwrap();
        assert_eq!(
            position_1.index,
            Decimal256::from_str("202.813614449380587585").unwrap()
        );
        assert_eq!(position_1.purchased, Uint256::from(202_813_614_449u128));
        assert_eq!(position_1.spent, Uint256::from(749_993_750u128));
        assert_eq!(position_1.in_balance, Uint256::from(250_006_250u128));

        let stream: StreamResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapQueryMsg::Stream {},
            )
            .unwrap();

        assert_eq!(
            stream.dist_index,
            Decimal256::from_str("202.813614449380587585").unwrap()
        );

        // Update position of subscriber 2
        app.set_block(BlockInfo {
            time: start_time.plus_seconds(3_575_000),
            height: 4,
            chain_id: "SS".to_string(),
        });
        let _res = app
            .execute_contract(
                test_accounts.subscriber_2.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &update_position_msg,
                &[],
            )
            .unwrap();

        let position_2: PositionResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapQueryMsg::Position {
                    owner: test_accounts.subscriber_2.to_string(),
                },
            )
            .unwrap();
        assert_eq!(
            position_2.index,
            Decimal256::from_str("238.074595237060799266").unwrap()
        );
        assert_eq!(position_2.purchased, Uint256::from(655_672_748_445u128));
        assert_eq!(position_2.spent, Uint256::from(2_673_076_923u128));
        assert_eq!(position_2.in_balance, Uint256::from(326_923_077u128));

        let stream: StreamResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapQueryMsg::Stream {},
            )
            .unwrap();

        assert_eq!(
            stream.dist_index,
            Decimal256::from_str("238.074595237060799266").unwrap()
        );

        // Update position after stream ends
        app.set_block(BlockInfo {
            time: end_time.plus_seconds(1),
            height: 5,
            chain_id: "SS".to_string(),
        });

        let _res = app
            .execute_contract(
                test_accounts.subscriber_1.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &update_position_msg,
                &[],
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
            stream.dist_index,
            Decimal256::from_str("264.137059297637397644").unwrap()
        );
        assert_eq!(stream.in_supply, Uint256::zero());

        let position_1: PositionResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapQueryMsg::Position {
                    owner: test_accounts.subscriber_1.to_string(),
                },
            )
            .unwrap();
        assert_eq!(
            position_1.index,
            Decimal256::from_str("264.137059297637397644").unwrap()
        );
        assert_eq!(position_1.spent, Uint256::from(1_000_000_000u128));
        assert_eq!(position_1.in_balance, Uint256::zero());

        // Update position after stream ends
        app.set_block(BlockInfo {
            time: end_time.plus_seconds(1),
            height: 6,
            chain_id: "SS".to_string(),
        });

        let _res = app
            .execute_contract(
                test_accounts.subscriber_2.clone(),
                Addr::unchecked(stream_swap_contract_address.clone()),
                &update_position_msg,
                &[],
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
            stream.dist_index,
            Decimal256::from_str("264.137059297637397644").unwrap()
        );
        assert_eq!(stream.in_supply, Uint256::zero());

        let position_2: PositionResponse = app
            .wrap()
            .query_wasm_smart(
                Addr::unchecked(stream_swap_contract_address.clone()),
                &StreamSwapQueryMsg::Position {
                    owner: test_accounts.subscriber_2.to_string(),
                },
            )
            .unwrap();
        assert_eq!(
            position_2.index,
            Decimal256::from_str("264.137059297637397644").unwrap()
        );
        assert_eq!(position_2.spent, Uint256::from(3_000_000_000u128));
        assert_eq!(position_2.in_balance, Uint256::zero());

        assert_eq!(stream.out_remaining, Uint256::zero());
        assert_eq!(
            position_1
                .purchased
                .checked_add(position_2.purchased)
                .unwrap(),
            // 1 difference due to rounding
            Uint256::from(stream.out_asset.amount.u128()).saturating_sub(Uint256::from(1u128))
        );
    }
}
