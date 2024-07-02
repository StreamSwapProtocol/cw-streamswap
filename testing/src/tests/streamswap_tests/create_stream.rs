#[cfg(test)]
mod create_stream_tests {
    use crate::helpers::{
        mock_messages::{get_create_stream_msg, get_factory_inst_msg},
        setup::{setup, SetupResponse},
    };
    use cosmwasm_std::{coin, Uint128};
    use cw_multi_test::Executor;
    use streamswap_factory::{
        error::ContractError as FactoryError, msg::QueryMsg, payment_checker::CustomPaymentError,
    };
    use streamswap_stream::{threshold::ThresholdError, ContractError as StreamSwapError};

    #[test]
    fn create_stream_failed_name_url_checks() {
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
        // Failed name checks
        // Name too short
        let create_stream_msg = get_create_stream_msg(
            "s",
            None,
            &test_accounts.creator_1.to_string(),
            coin(100, "out_denom"),
            "in_denom",
            app.block_info().time.plus_seconds(100).into(),
            app.block_info().time.plus_seconds(200).into(),
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
            .unwrap_err();

        let err = res.source().unwrap().source().unwrap();
        let error = err.downcast_ref::<StreamSwapError>().unwrap();
        assert_eq!(*error, StreamSwapError::StreamNameTooShort {});
        // Name too long
        let long_name = "a".repeat(65);
        let create_stream_msg = get_create_stream_msg(
            &long_name,
            None,
            &test_accounts.creator_1.to_string(),
            coin(100, "out_denom"),
            "in_denom",
            app.block_info().time.plus_seconds(100).into(),
            app.block_info().time.plus_seconds(200).into(),
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
            .unwrap_err();

        let err = res.source().unwrap().source().unwrap();
        let error = err.downcast_ref::<StreamSwapError>().unwrap();
        assert_eq!(*error, StreamSwapError::StreamNameTooLong {});

        // Invalid name
        let create_stream_msg = get_create_stream_msg(
            "abc~ß",
            None,
            &test_accounts.creator_1.to_string(),
            coin(100, "out_denom"),
            "in_denom",
            app.block_info().time.plus_seconds(100).into(),
            app.block_info().time.plus_seconds(200).into(),
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
            .unwrap_err();

        let err = res.source().unwrap().source().unwrap();
        let error = err.downcast_ref::<StreamSwapError>().unwrap();
        assert_eq!(*error, StreamSwapError::InvalidStreamName {});

        // Failed url checks
        // URL too short
        let create_stream_msg = get_create_stream_msg(
            "stream",
            Some("a".to_string()),
            &test_accounts.creator_1.to_string(),
            coin(100, "out_denom"),
            "in_denom",
            app.block_info().time.plus_seconds(100).into(),
            app.block_info().time.plus_seconds(200).into(),
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
            .unwrap_err();

        let err = res.source().unwrap().source().unwrap();
        let error = err.downcast_ref::<StreamSwapError>().unwrap();

        assert_eq!(*error, StreamSwapError::StreamUrlTooShort {});

        // URL too long
        let long_url = "a".repeat(256);
        let create_stream_msg = get_create_stream_msg(
            "stream",
            Some(long_url),
            &test_accounts.creator_1.to_string(),
            coin(100, "out_denom"),
            "in_denom",
            app.block_info().time.plus_seconds(100).into(),
            app.block_info().time.plus_seconds(200).into(),
            Some(Uint128::from(100u128)),
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
            .unwrap_err();

        let err = res.source().unwrap().source().unwrap();
        let error = err.downcast_ref::<StreamSwapError>().unwrap();
        assert_eq!(*error, StreamSwapError::StreamUrlTooLong {});
    }

    #[test]
    fn create_stream_failed_fund_checks() {
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

        // Non permissioned in denom
        let create_stream_msg = get_create_stream_msg(
            "stream",
            None,
            &test_accounts.creator_1.to_string(),
            coin(100, "out_denom"),
            "invalid_in_denom",
            app.block_info().time.plus_seconds(100).into(),
            app.block_info().time.plus_seconds(200).into(),
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
            .unwrap_err();
        let err = res.source().unwrap();
        let error = err.downcast_ref::<FactoryError>().unwrap();
        assert_eq!(*error, FactoryError::InDenomIsNotAccepted {});

        // Same in and out denom
        let create_stream_msg = get_create_stream_msg(
            "stream",
            None,
            &test_accounts.creator_1.to_string(),
            coin(100, "in_denom"),
            "in_denom",
            app.block_info().time.plus_seconds(100).into(),
            app.block_info().time.plus_seconds(200).into(),
            None,
            None,
            None,
        );
        let res = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                factory_address.clone(),
                &create_stream_msg,
                &[coin(100, "fee_denom"), coin(100, "in_denom")],
            )
            .unwrap_err();

        let err = res.source().unwrap().source().unwrap();
        let error = err.downcast_ref::<StreamSwapError>().unwrap();
        assert_eq!(*error, StreamSwapError::SameDenomOnEachSide {});

        // Zero out supply
        let create_stream_msg = get_create_stream_msg(
            "stream",
            None,
            &test_accounts.creator_1.to_string(),
            coin(0, "out_denom"),
            "in_denom",
            app.block_info().time.plus_seconds(100).into(),
            app.block_info().time.plus_seconds(200).into(),
            Some(Uint128::from(100u128)),
            None,
            None,
        );

        let res = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                factory_address.clone(),
                &create_stream_msg,
                &[coin(100, "fee_denom")],
            )
            .unwrap_err();
        println!("{:?}", res);

        let err = res.source().unwrap().source().unwrap();
        let error = err.downcast_ref::<StreamSwapError>().unwrap();
        assert_eq!(*error, StreamSwapError::ZeroOutSupply {});

        // No funds sent
        let create_stream_msg = get_create_stream_msg(
            "stream",
            None,
            &test_accounts.creator_1.to_string(),
            coin(100, "out_denom"),
            "in_denom",
            app.block_info().time.plus_seconds(100).into(),
            app.block_info().time.plus_seconds(200).into(),
            None,
            None,
            None,
        );

        let res = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                factory_address.clone(),
                &create_stream_msg,
                &[coin(100, "fee_denom")],
            )
            .unwrap_err();

        let err = res.source().unwrap();
        let error = err.downcast_ref::<FactoryError>().unwrap();
        assert_eq!(
            *error,
            FactoryError::CustomPayment(CustomPaymentError::InsufficientFunds {
                expected: [coin(100, "fee_denom"), coin(100, "out_denom")].to_vec(),
                actual: [coin(100, "fee_denom")].to_vec()
            })
        );

        // Insufficient fee
        let create_stream_msg = get_create_stream_msg(
            "stream",
            None,
            &test_accounts.creator_1.to_string(),
            coin(100, "out_denom"),
            "in_denom",
            app.block_info().time.plus_seconds(100).into(),
            app.block_info().time.plus_seconds(200).into(),
            None,
            None,
            None,
        );

        let res = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                factory_address.clone(),
                &create_stream_msg,
                &[coin(99, "fee_denom"), coin(100, "out_denom")],
            )
            .unwrap_err();

        let err = res.source().unwrap();
        let error = err.downcast_ref::<FactoryError>().unwrap();
        assert_eq!(
            *error,
            FactoryError::CustomPayment(CustomPaymentError::InsufficientFunds {
                expected: [coin(100, "fee_denom"), coin(100, "out_denom")].to_vec(),
                actual: [coin(99, "fee_denom"), coin(100, "out_denom")].to_vec()
            })
        );

        // Extra funds sent
        let create_stream_msg = get_create_stream_msg(
            "stream",
            None,
            &test_accounts.creator_1.to_string(),
            coin(100, "out_denom"),
            "in_denom",
            app.block_info().time.plus_seconds(100).into(),
            app.block_info().time.plus_seconds(200).into(),
            None,
            None,
            None,
        );

        let res = app
            .execute_contract(
                test_accounts.creator_1.clone(),
                factory_address.clone(),
                &create_stream_msg,
                &[
                    coin(100, "fee_denom"),
                    coin(100, "out_denom"),
                    coin(100, "wrong_denom"),
                ],
            )
            .unwrap_err();

        let err = res.source().unwrap();
        let error = err.downcast_ref::<FactoryError>().unwrap();
        assert_eq!(
            *error,
            FactoryError::CustomPayment(CustomPaymentError::InsufficientFunds {
                expected: [coin(100, "fee_denom"), coin(100, "out_denom")].to_vec(),
                actual: [
                    coin(100, "fee_denom"),
                    coin(100, "out_denom"),
                    coin(100, "wrong_denom")
                ]
                .to_vec()
            })
        );

        // Threshold zero
        let create_stream_msg = get_create_stream_msg(
            "stream",
            None,
            &test_accounts.creator_1.to_string(),
            coin(100, "out_denom"),
            "in_denom",
            app.block_info().time.plus_seconds(100).into(),
            app.block_info().time.plus_seconds(200).into(),
            Some(Uint128::from(0u128)),
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
            .unwrap_err();

        let err = res.source().unwrap().source().unwrap();
        let error = err.downcast_ref::<StreamSwapError>().unwrap();
        assert_eq!(
            *error,
            StreamSwapError::ThresholdError(ThresholdError::ThresholdZero {})
        );
    }

    #[test]
    fn create_stream_failed_duration_checks() {
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

        let create_stream_msg = get_create_stream_msg(
            "stream",
            None,
            &test_accounts.creator_1.to_string(),
            coin(100, "out_denom"),
            "in_denom",
            app.block_info().time.plus_seconds(200).into(),
            app.block_info().time.plus_seconds(100).into(),
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
            .unwrap_err();
        let err = res.source().unwrap();
        let error = err.downcast_ref::<FactoryError>().unwrap();
        assert_eq!(*error, FactoryError::StreamInvalidEndTime {});

        // Now time > start time
        let create_stream_msg = get_create_stream_msg(
            "stream",
            None,
            &test_accounts.creator_1.to_string(),
            coin(100, "out_denom"),
            "in_denom",
            app.block_info().time.minus_seconds(1),
            app.block_info().time.plus_seconds(200).into(),
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
            .unwrap_err();
        let err = res.source().unwrap();
        let error = err.downcast_ref::<FactoryError>().unwrap();
        assert_eq!(*error, FactoryError::StreamInvalidStartTime {});

        // Stream duration too short
        let create_stream_msg = get_create_stream_msg(
            "stream",
            None,
            &test_accounts.creator_1.to_string(),
            coin(100, "out_denom"),
            "in_denom",
            app.block_info().time.plus_seconds(100).into(),
            app.block_info().time.plus_seconds(101).into(),
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
            .unwrap_err();

        let err = res.source().unwrap();
        let error = err.downcast_ref::<FactoryError>().unwrap();
        assert_eq!(*error, FactoryError::StreamDurationTooShort {});

        // Stream starts too soon
        let create_stream_msg = get_create_stream_msg(
            "stream",
            None,
            &test_accounts.creator_1.to_string(),
            coin(100, "out_denom"),
            "in_denom",
            app.block_info().time.plus_seconds(1),
            app.block_info().time.plus_seconds(200).into(),
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
            .unwrap_err();

        let err = res.source().unwrap();
        let error = err.downcast_ref::<FactoryError>().unwrap();
        assert_eq!(*error, FactoryError::StreamStartsTooSoon {});
    }

    #[test]
    fn create_stream_happy_path() {
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

        let create_stream_msg = get_create_stream_msg(
            "stream",
            None,
            &test_accounts.creator_1.to_string(),
            coin(100, "out_denom"),
            "in_denom",
            app.block_info().time.plus_seconds(100).into(),
            app.block_info().time.plus_seconds(200).into(),
            Some(Uint128::from(100u128)),
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

        // Query stream with id
        let query_msg = QueryMsg::LastStreamId {};
        let res: u32 = app
            .wrap()
            .query_wasm_smart(factory_address, &query_msg)
            .unwrap();
        assert_eq!(res, 1);
    }
}
