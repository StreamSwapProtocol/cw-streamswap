#[cfg(test)]
mod test_module {
    use crate::contract::execute;
    use crate::contract::{
        execute_create_stream, execute_exit_stream, execute_finalize_stream,
        execute_update_operator, execute_update_position, execute_update_stream, instantiate,
        query_average_price, query_config, query_last_streamed_price, query_position, query_stream,
    };
    use crate::killswitch::{execute_pause_stream, execute_withdraw_paused, sudo_resume_stream};
    use crate::msg::ExecuteMsg::UpdateProtocolAdmin;
    use crate::state::{Status, Stream};
    use crate::threshold::ThresholdError;
    use crate::ContractError;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::StdError::{self};
    use cosmwasm_std::{
        attr, coin, Addr, BankMsg, Coin, CosmosMsg, Decimal, Decimal256, Response, SubMsg,
        Timestamp, Uint128, Uint256, Uint64,
    };
    use cw_utils::PaymentError;
    use std::ops::Sub;
    use std::str::FromStr;

    #[test]
    fn test_compute_shares_amount() {
        let mut stream = Stream::new(
            "test".to_string(),
            Addr::unchecked("treasury"),
            Some("url".to_string()),
            "out_denom".to_string(),
            Uint256::from(100u128),
            "in_denom".to_string(),
            Timestamp::from_seconds(0),
            Timestamp::from_seconds(100),
            Timestamp::from_seconds(0),
            "fee".to_string(),
            Uint128::from(100u128),
            Decimal256::percent(10),
        );

        // add new shares
        let shares = stream.compute_shares_amount(Uint256::from(100u128), false);
        assert_eq!(shares, Uint256::from(100u128));
        stream.in_supply = Uint256::from(100u128);
        stream.shares = shares;

        // add new shares
        stream.shares += stream.compute_shares_amount(Uint256::from(100u128), false);
        stream.in_supply += Uint256::from(100u128);
        assert_eq!(stream.shares, Uint256::from(200u128));

        // add new shares
        stream.shares += stream.compute_shares_amount(Uint256::from(250u128), false);
        assert_eq!(stream.shares, Uint256::from(450u128));
        stream.in_supply += Uint256::from(250u128);

        // remove shares
        stream.shares -= stream.compute_shares_amount(Uint256::from(100u128), true);
        assert_eq!(stream.shares, Uint256::from(350u128));
        stream.in_supply -= Uint256::from(100u128);
    }

    #[test]
    fn test_create_stream() {
        let mut deps = mock_dependencies();
        // Invalid exit fee
        let msg = crate::msg::InstantiateMsg {
            min_stream_seconds: Uint64::new(1000),
            min_seconds_until_start_time: Uint64::new(1000),
            stream_creation_denom: "fee".to_string(),
            stream_creation_fee: Uint128::new(100),
            exit_fee_percent: Decimal256::percent(101),
            fee_collector: "collector".to_string(),
            protocol_admin: "protocol_admin".to_string(),
            accepted_in_denom: "in".to_string(),
        };
        let res =
            instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap_err();
        assert_eq!(res, ContractError::InvalidExitFeePercent {});

        // Invalid stream creation fee
        let msg = crate::msg::InstantiateMsg {
            min_stream_seconds: Uint64::new(1000),
            min_seconds_until_start_time: Uint64::new(1000),
            stream_creation_denom: "fee".to_string(),
            stream_creation_fee: Uint128::zero(),
            exit_fee_percent: Decimal256::percent(1),
            fee_collector: "collector".to_string(),
            protocol_admin: "protocol_admin".to_string(),
            accepted_in_denom: "in".to_string(),
        };
        let res =
            instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap_err();
        assert_eq!(res, ContractError::InvalidStreamCreationFee {});

        let msg = crate::msg::InstantiateMsg {
            min_stream_seconds: Uint64::new(1000),
            min_seconds_until_start_time: Uint64::new(1000),
            stream_creation_denom: "fee".to_string(),
            stream_creation_fee: Uint128::new(100),
            exit_fee_percent: Decimal256::percent(1),
            fee_collector: "collector".to_string(),
            protocol_admin: "protocol_admin".to_string(),
            accepted_in_denom: "in".to_string(),
        };
        instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

        // invalid in_denom
        let treasury = "treasury";
        let name = "name";
        let url = "https://sample.url";
        let start_time = Timestamp::from_seconds(3000);
        let end_time = Timestamp::from_seconds(100000);
        let out_supply = Uint256::from(50_000_000u128);
        let out_denom = "out_denom";
        let in_denom = "random";

        let info = mock_info("creator", &[]);
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(0);
        let res = execute_create_stream(
            deps.as_mut(),
            env,
            info,
            treasury.to_string(),
            name.to_string(),
            Some(url.to_string()),
            in_denom.to_string(),
            out_denom.to_string(),
            out_supply,
            start_time,
            end_time,
            None,
        );
        assert_eq!(res, Err(ContractError::InDenomIsNotAccepted {}));
        // end < start case
        let treasury = "treasury";
        let name = "name";
        let url = "https://sample.url";
        let start_time = Timestamp::from_seconds(1000);
        let end_time = Timestamp::from_seconds(10);
        let out_supply = Uint256::from(50_000_000u128);
        let out_denom = "out_denom";
        let in_denom = "in";

        let info = mock_info("creator", &[]);
        let env = mock_env();
        let res = execute_create_stream(
            deps.as_mut(),
            env,
            info,
            treasury.to_string(),
            name.to_string(),
            Some(url.to_string()),
            in_denom.to_string(),
            out_denom.to_string(),
            out_supply,
            start_time,
            end_time,
            None,
        );
        assert_eq!(res, Err(ContractError::StreamInvalidEndTime {}));

        // min_stream_duration is not sufficient
        let end_time = Timestamp::from_seconds(1000);
        let start_time = Timestamp::from_seconds(500);
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(0);
        let info = mock_info("creator1", &[]);
        let res = execute_create_stream(
            deps.as_mut(),
            env,
            info,
            treasury.to_string(),
            name.to_string(),
            Some(url.to_string()),
            in_denom.to_string(),
            out_denom.to_string(),
            out_supply,
            start_time,
            end_time,
            None,
        );
        assert_eq!(res, Err(ContractError::StreamDurationTooShort {}));

        // start cannot be before current time
        let end_time = Timestamp::from_seconds(1000);
        let start_time = Timestamp::from_seconds(500);
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(600);
        let info = mock_info("creator1", &[]);
        let res = execute_create_stream(
            deps.as_mut(),
            env,
            info,
            treasury.to_string(),
            name.to_string(),
            Some(url.to_string()),
            in_denom.to_string(),
            out_denom.to_string(),
            out_supply,
            start_time,
            end_time,
            None,
        );
        assert_eq!(res, Err(ContractError::StreamInvalidStartTime {}));

        // stream starts too soon case
        let end_time = Timestamp::from_seconds(100000);
        let start_time = Timestamp::from_seconds(1400);
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(401);
        let info = mock_info("creator1", &[]);
        let res = execute_create_stream(
            deps.as_mut(),
            env,
            info,
            treasury.to_string(),
            name.to_string(),
            Some(url.to_string()),
            in_denom.to_string(),
            out_denom.to_string(),
            out_supply,
            start_time,
            end_time,
            None,
        );
        assert_eq!(res, Err(ContractError::StreamStartsTooSoon {}));

        // Same in and out denom case
        let end_time = Timestamp::from_seconds(100000);
        let start_time = Timestamp::from_seconds(3000);
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(1);
        let info = mock_info("creator1", &[]);
        let res = execute_create_stream(
            deps.as_mut(),
            env,
            info,
            treasury.to_string(),
            name.to_string(),
            Some(url.to_string()),
            "in".to_string(),
            "in".to_string(),
            out_supply,
            start_time,
            end_time,
            None,
        );
        assert_eq!(res, Err(ContractError::SameDenomOnEachSide {}));

        // 0 out supply case
        let end_time = Timestamp::from_seconds(100000);
        let start_time = Timestamp::from_seconds(3000);
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(1);
        let info = mock_info("creator1", &[]);
        let res = execute_create_stream(
            deps.as_mut(),
            env,
            info,
            treasury.to_string(),
            name.to_string(),
            Some(url.to_string()),
            in_denom.to_string(),
            out_denom.to_string(),
            Uint256::zero(),
            start_time,
            end_time,
            None,
        );
        assert_eq!(res, Err(ContractError::ZeroOutSupply {}));

        // threshold zero case
        let start_time = Timestamp::from_seconds(3000);
        let end_time = Timestamp::from_seconds(100000);
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(1);
        let info = mock_info(
            "creator1",
            &[
                Coin {
                    denom: "fee".to_string(),
                    amount: Uint128::new(100),
                },
                Coin {
                    denom: out_denom.to_string(),
                    amount: out_supply.to_string().parse().unwrap(),
                },
            ],
        );
        let res = execute_create_stream(
            deps.as_mut(),
            env,
            info,
            treasury.to_string(),
            name.to_string(),
            Some(url.to_string()),
            in_denom.to_string(),
            out_denom.to_string(),
            out_supply,
            start_time,
            end_time,
            Some(Uint256::zero()),
        )
        .unwrap_err();
        assert_eq!(
            res,
            ContractError::ThresholdError(ThresholdError::ThresholdZero {})
        );

        // no funds fee case
        let end_time = Timestamp::from_seconds(100000);
        let start_time = Timestamp::from_seconds(3000);
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(1);
        let info = mock_info("creator1", &[]);
        let res = execute_create_stream(
            deps.as_mut(),
            env,
            info,
            treasury.to_string(),
            name.to_string(),
            Some(url.to_string()),
            in_denom.to_string(),
            out_denom.to_string(),
            out_supply,
            start_time,
            end_time,
            None,
        );
        assert_eq!(res, Err(ContractError::NoFundsSent {}));

        // wrong supply amount case
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(1);
        let info = mock_info("creator1", &[Coin::new(1_000_000, "out_denom")]);
        let res = execute_create_stream(
            deps.as_mut(),
            env,
            info,
            treasury.to_string(),
            name.to_string(),
            Some(url.to_string()),
            in_denom.to_string(),
            out_denom.to_string(),
            out_supply,
            start_time,
            end_time,
            None,
        );
        assert_eq!(res, Err(ContractError::StreamOutSupplyFundsRequired {}));

        // wrong creation fee case
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(1);
        let info = mock_info(
            "creator1",
            &[
                Coin::new(out_supply.to_string().parse().unwrap(), "out_denom"),
                Coin::new(99, "fee"),
            ],
        );
        let res = execute_create_stream(
            deps.as_mut(),
            env,
            info,
            treasury.to_string(),
            name.to_string(),
            Some(url.to_string()),
            in_denom.to_string(),
            out_denom.to_string(),
            out_supply,
            start_time,
            end_time,
            None,
        );
        assert_eq!(res, Err(ContractError::StreamCreationFeeRequired {}));

        // no creation fee case
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(1);
        let info = mock_info(
            "creator1",
            &[Coin::new(
                out_supply.to_string().parse().unwrap(),
                "out_denom",
            )],
        );
        let res = execute_create_stream(
            deps.as_mut(),
            env,
            info,
            treasury.to_string(),
            name.to_string(),
            Some(url.to_string()),
            in_denom.to_string(),
            out_denom.to_string(),
            out_supply,
            start_time,
            end_time,
            None,
        );
        assert_eq!(res, Err(ContractError::NoFundsSent {}));

        // mismatch creation fee case
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(1);
        let info = mock_info(
            "creator1",
            &[Coin::new(
                out_supply.to_string().parse().unwrap(),
                "out_denom",
            )],
        );
        let res = execute_create_stream(
            deps.as_mut(),
            env,
            info,
            treasury.to_string(),
            name.to_string(),
            Some(url.to_string()),
            in_denom.to_string(),
            out_denom.to_string(),
            out_supply,
            start_time,
            end_time,
            None,
        );
        assert_eq!(res, Err(ContractError::NoFundsSent {}));

        // same denom case, insufficient total
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(1);
        let info = mock_info("creator1", &[Coin::new(1, "fee")]);
        let res = execute_create_stream(
            deps.as_mut(),
            env,
            info,
            treasury.to_string(),
            name.to_string(),
            Some(url.to_string()),
            in_denom.to_string(),
            "fee".to_string(),
            out_supply,
            start_time,
            end_time,
            None,
        );
        assert_eq!(res, Err(ContractError::StreamOutSupplyFundsRequired {}));

        // same denom case, sufficient total
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(1);
        let info = mock_info(
            "creator1",
            &[Coin::new(
                out_supply
                    .strict_add(Uint256::from(100u128))
                    .to_string()
                    .parse()
                    .unwrap(),
                "fee",
            )],
        );
        execute_create_stream(
            deps.as_mut(),
            env,
            info,
            treasury.to_string(),
            name.to_string(),
            Some(url.to_string()),
            in_denom.to_string(),
            "fee".to_string(),
            out_supply,
            start_time,
            end_time,
            None,
        )
        .unwrap();

        // same tokens extra funds sent
        let info = mock_info(
            "creator1",
            &[
                coin(
                    out_supply
                        .strict_add(Uint256::from(100u128))
                        .to_string()
                        .parse()
                        .unwrap(),
                    "fee",
                ),
                coin(15, "random"),
            ],
        );
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(1);
        let err = execute_create_stream(
            deps.as_mut(),
            env,
            info,
            treasury.to_string(),
            name.to_string(),
            Some(url.to_string()),
            in_denom.to_string(),
            "fee".to_string(),
            out_supply,
            start_time,
            end_time,
            None,
        )
        .unwrap_err();
        assert_eq!(err, ContractError::InvalidFunds {});

        // different tokens extra funds sent
        let info = mock_info(
            "creator1",
            &[
                coin(out_supply.to_string().parse().unwrap(), "different_denom"),
                coin(Uint128::new(100).u128(), "fee"),
                coin(15, "random"),
            ],
        );
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(1);
        let err = execute_create_stream(
            deps.as_mut(),
            env,
            info,
            treasury.to_string(),
            name.to_string(),
            Some(url.to_string()),
            in_denom.to_string(),
            "different_denom".to_string(),
            out_supply,
            start_time,
            end_time,
            None,
        )
        .unwrap_err();
        assert_eq!(err, ContractError::InvalidFunds {});

        // failed name checks
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(1);
        let info = mock_info(
            "creator1",
            &[
                Coin::new(out_supply.to_string().parse().unwrap(), "out_denom"),
                Coin::new(100, "fee"),
            ],
        );
        let res = execute_create_stream(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            treasury.to_string(),
            "n".to_string(),
            Some(url.to_string()),
            in_denom.to_string(),
            out_denom.to_string(),
            out_supply,
            start_time,
            end_time,
            None,
        )
        .unwrap_err();
        assert_eq!(res, ContractError::StreamNameTooShort {});

        let res = execute_create_stream(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            treasury.to_string(),
            "12345678901234567890123456789012345678901234567890123456789012345".to_string(),
            Some(url.to_string()),
            in_denom.to_string(),
            out_denom.to_string(),
            out_supply,
            start_time,
            end_time,
            None,
        )
        .unwrap_err();
        assert_eq!(res, ContractError::StreamNameTooLong {});

        let res = execute_create_stream(
            deps.as_mut(),
            env,
            info,
            treasury.to_string(),
            "abc~ß".to_string(),
            Some(url.to_string()),
            in_denom.to_string(),
            out_denom.to_string(),
            out_supply,
            start_time,
            end_time,
            None,
        )
        .unwrap_err();
        assert_eq!(res, ContractError::InvalidStreamName {});

        //failed url checks
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(1);
        let info = mock_info(
            "creator1",
            &[
                Coin::new(out_supply.to_string().parse().unwrap(), "out_denom"),
                Coin::new(100, "fee"),
            ],
        );
        let res = execute_create_stream(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            treasury.to_string(),
            "name".to_string(),
            Some("https://a.b".to_string()),
            in_denom.to_string(),
            out_denom.to_string(),
            out_supply,
            start_time,
            end_time,
            None,
        )
        .unwrap_err();
        assert_eq!(res, ContractError::StreamUrlTooShort {});

        let res = execute_create_stream(
            deps.as_mut(),
            env.clone(),
            info.clone(),
            treasury.to_string(),
            "name".to_string(),
            Some("https://abcdefghijklmnopqrstuvw.xyz/abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz/abcdefghijklmnopqrstuvwxyzabcdefghijklmn".to_string()),
            in_denom.to_string(),
            out_denom.to_string(),
            out_supply,
            start_time,
            end_time,
            None,
        )
            .unwrap_err();
        assert_eq!(res, ContractError::StreamUrlTooLong {});

        let res = execute_create_stream(
            deps.as_mut(),
            env,
            info,
            treasury.to_string(),
            "name".to_string(),
            Some("https://abc defghijklmnopqrstuvw.xyz/".to_string()),
            in_denom.to_string(),
            out_denom.to_string(),
            out_supply,
            start_time,
            end_time,
            None,
        )
        .unwrap_err();

        assert_eq!(res, ContractError::InvalidStreamUrl {});

        // happy path
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(1);
        let info = mock_info(
            "creator1",
            &[
                Coin::new(out_supply.to_string().parse().unwrap(), "out_denom"),
                Coin::new(100, "fee"),
            ],
        );
        execute_create_stream(
            deps.as_mut(),
            env,
            info,
            treasury.to_string(),
            name.to_string(),
            Some(url.to_string()),
            in_denom.to_string(),
            out_denom.to_string(),
            out_supply,
            start_time,
            end_time,
            None,
        )
        .unwrap();

        // query stream with id
        let env = mock_env();
        let stream = query_stream(deps.as_ref(), env, 1).unwrap();
        assert_eq!(stream.id, 1);
    }

    #[test]
    fn test_subscribe() {
        let treasury = Addr::unchecked("treasury");
        let start = Timestamp::from_seconds(2000);
        let end = Timestamp::from_seconds(1_000_000);
        let out_supply = Uint256::from(1_000_000u128);
        let out_denom = "out_denom";

        // instantiate
        let mut deps = mock_dependencies();
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(100);
        let msg = crate::msg::InstantiateMsg {
            min_stream_seconds: Uint64::new(1000),
            min_seconds_until_start_time: Uint64::new(1000),
            stream_creation_denom: "fee".to_string(),
            stream_creation_fee: Uint128::new(100),
            exit_fee_percent: Decimal256::percent(1),
            fee_collector: "collector".to_string(),
            protocol_admin: "protocol_admin".to_string(),
            accepted_in_denom: "in".to_string(),
        };
        instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

        // create stream
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(1);
        let info = mock_info(
            "creator1",
            &[
                Coin::new(out_supply.to_string().parse().unwrap(), out_denom),
                Coin::new(100, "fee"),
            ],
        );
        execute_create_stream(
            deps.as_mut(),
            env,
            info,
            treasury.to_string(),
            "test".to_string(),
            Some("https://sample.url".to_string()),
            "in".to_string(),
            out_denom.to_string(),
            out_supply,
            start,
            end,
            None,
        )
        .unwrap();

        // stream ended
        let mut env = mock_env();
        env.block.time = end.plus_seconds(1000000);
        let info = mock_info("creator1", &[]);
        let msg = crate::msg::ExecuteMsg::Subscribe {
            stream_id: 1,
            operator_target: None,
            operator: None,
        };
        let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(res, ContractError::StreamEnded {});

        // no funds
        let mut env = mock_env();
        env.block.time = start.plus_seconds(100);
        let info = mock_info("creator1", &[]);
        let msg = crate::msg::ExecuteMsg::Subscribe {
            stream_id: 1,
            operator_target: None,
            operator: None,
        };
        let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(res, PaymentError::NoFunds {}.into());

        // incorrect denom
        let mut env = mock_env();
        env.block.time = start.plus_seconds(100);
        let info = mock_info("creator1", &[Coin::new(100, "wrong_denom")]);
        let msg = crate::msg::ExecuteMsg::Subscribe {
            stream_id: 1,
            operator_target: None,
            operator: None,
        };
        let res = execute(deps.as_mut(), env.clone(), info, msg).unwrap_err();
        assert_eq!(res, PaymentError::MissingDenom("in".to_string()).into());

        let stream = query_stream(deps.as_ref(), env, 1).unwrap();
        assert_eq!(stream.status, Status::Waiting);

        // first subscribe
        let mut env = mock_env();
        env.block.time = start.plus_seconds(100);
        let info = mock_info("creator1", &[Coin::new(1_000_000, "in")]);
        let msg = crate::msg::ExecuteMsg::Subscribe {
            stream_id: 1,
            operator_target: None,
            operator: None,
        };
        let _res = execute(deps.as_mut(), env, info, msg);

        // dist index updated
        let env = mock_env();
        let stream = query_stream(deps.as_ref(), env.clone(), 1).unwrap();
        // position index not updated,  in_supply updated
        assert_eq!(stream.dist_index, Decimal256::zero());
        //see that the status is updated
        assert_eq!(stream.status, Status::Active);
        assert_eq!(stream.in_supply, Uint256::from(1000000u128));
        let position = query_position(deps.as_ref(), env, 1, "creator1".to_string()).unwrap();
        assert_eq!(position.index, Decimal256::zero());
        assert_eq!(position.in_balance, Uint256::from(1000000u128));
        // unauthorized subscription increase
        let mut env = mock_env();
        env.block.time = start.plus_seconds(200);
        let info = mock_info("random", &[Coin::new(1_000_000, "in")]);
        let msg = crate::msg::ExecuteMsg::Subscribe {
            stream_id: 1,
            operator_target: Some("creator1".to_string()),
            operator: None,
        };
        let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(res, ContractError::Unauthorized {});

        // subscription increase
        let mut env = mock_env();
        env.block.time = start.plus_seconds(200);
        let info = mock_info("creator1", &[Coin::new(1_000_000, "in")]);
        let msg = crate::msg::ExecuteMsg::Subscribe {
            stream_id: 1,
            operator_target: None,
            operator: None,
        };
        let _res = execute(deps.as_mut(), env.clone(), info, msg);
        // dist index updated
        let stream = query_stream(deps.as_ref(), env.clone(), 1).unwrap();
        assert_eq!(stream.dist_index, Decimal256::from_str("0.0001").unwrap());
        // dist index updated, position reduced and increased
        let position = query_position(deps.as_ref(), env, 1, "creator1".to_string()).unwrap();
        assert_eq!(position.index, Decimal256::from_str("0.0001").unwrap());
        assert_eq!(position.in_balance, Uint256::from(1999900u128));
    }

    #[test]
    fn test_subscribe_pending() {
        // instantiate
        let treasury = Addr::unchecked("treasury");
        let start = Timestamp::from_seconds(5_000);
        let end = Timestamp::from_seconds(10_000);
        let out_supply = Uint256::from(1_000_000u128);
        let out_denom = "out_denom";

        // instantiate
        let mut deps = mock_dependencies();
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(100);
        let msg = crate::msg::InstantiateMsg {
            min_stream_seconds: Uint64::new(1000),
            min_seconds_until_start_time: Uint64::new(1000),
            stream_creation_denom: "fee".to_string(),
            stream_creation_fee: Uint128::new(100),
            exit_fee_percent: Decimal256::percent(1),
            fee_collector: "collector".to_string(),
            protocol_admin: "protocol_admin".to_string(),
            accepted_in_denom: "in".to_string(),
        };
        instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

        // create stream
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(200);
        let info = mock_info(
            "creator1",
            &[
                Coin::new(out_supply.to_string().parse().unwrap(), out_denom),
                Coin::new(100, "fee"),
            ],
        );
        execute_create_stream(
            deps.as_mut(),
            env,
            info,
            treasury.to_string(),
            "test".to_string(),
            Some("https://sample.url".to_string()),
            "in".to_string(),
            out_denom.to_string(),
            out_supply,
            start,
            end,
            None,
        )
        .unwrap();

        // first subscribe
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(300);

        let info = mock_info("creator1", &[Coin::new(1_000_000, "in")]);
        let msg = crate::msg::ExecuteMsg::Subscribe {
            stream_id: 1,
            operator_target: None,
            operator: None,
        };
        let res = execute(deps.as_mut(), env, info, msg).unwrap();
        assert_eq!(res.attributes[0].key, "action");
        assert_eq!(res.attributes[0].value, "subscribe_pending");
        // query stream
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(350);
        let stream = query_stream(deps.as_ref(), env, 1).unwrap();
        assert_eq!(stream.status, Status::Waiting);
        assert_eq!(stream.in_supply, Uint256::from(1000000u128));
        assert_eq!(stream.shares, Uint256::from(1000000u128));

        // second subscribe still waiting
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(500);
        let info = mock_info("creator1", &[Coin::new(1_000_000, "in")]);
        let msg = crate::msg::ExecuteMsg::Subscribe {
            stream_id: 1,
            operator_target: None,
            operator: None,
        };
        let res = execute(deps.as_mut(), env, info, msg).unwrap();
        assert_eq!(res.attributes[0].key, "action");
        assert_eq!(res.attributes[0].value, "subscribe_pending");

        // query stream
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(450);
        let stream = query_stream(deps.as_ref(), env, 1).unwrap();
        assert_eq!(stream.status, Status::Waiting);
        assert_eq!(stream.in_supply, Uint256::from(2000000u128));

        // Before stream start time 2 subscriptions have been made and the stream is pending
        // After stream start time plus 1000 seconds one subscription is made and the stream is active
        // Creator 1 has 2 subscriptions and 2_000_000 in balance
        // Creator 2 has 1 subscription and 1_000_000 in balance
        // At 6000 seconds the stream is active and the balance to be distributed is ~2000000
        // At 6000 seconds creator 1 shold spent 2000000*1000/5000= 400000
        // At 6000 seconds creator 1 should get all 2000000 tokens
        // At 6000 seconds creator 2 should get 0 tokens
        // At 7500 seconds the stream is active and the balance to be distributed is 300000
        // At 7500 seconds creator 1 should get 300000*2000000/3250000 = 184615
        // At 7500 seconds creator 2 should get 300000*1250000/3250000 = 115384

        // subscription after start time
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(6000);
        let info = mock_info("creator2", &[Coin::new(1_000_000, "in")]);
        let msg = crate::msg::ExecuteMsg::Subscribe {
            stream_id: 1,
            operator_target: None,
            operator: None,
        };
        let res = execute(deps.as_mut(), env, info, msg).unwrap();
        assert_eq!(res.attributes[0].key, "action");
        // diffirent action because stream is active
        assert_eq!(res.attributes[0].value, "subscribe");

        // update creator 1 position
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(6000);
        let update_msg = crate::msg::ExecuteMsg::UpdatePosition {
            stream_id: 1,
            operator_target: None,
        };
        let info = mock_info("creator1", &[]);
        let _res = execute(deps.as_mut(), env.clone(), info, update_msg).unwrap();
        let position = query_position(deps.as_ref(), env, 1, "creator1".to_string()).unwrap();
        assert_eq!(position.spent, Uint256::from(400000u128));

        // query stream
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(6000);
        let stream = query_stream(deps.as_ref(), env, 1).unwrap();
        assert_eq!(stream.status, Status::Active);
        assert_eq!(stream.in_supply, Uint256::from(3000000u128 - 400000u128));
        assert_eq!(stream.spent_in, Uint256::from(400000u128));

        // update creator 1 position at 3500
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(7500);
        let update_msg = crate::msg::ExecuteMsg::UpdatePosition {
            stream_id: 1,
            operator_target: None,
        };
        let info = mock_info("creator1", &[]);
        let _res = execute(deps.as_mut(), env.clone(), info, update_msg).unwrap();

        // query position
        let res = query_position(deps.as_ref(), env, 1, "creator1".to_string()).unwrap();
        assert_eq!(res.purchased, Uint256::from(184615u128 + 200000u128));
        assert_eq!(res.spent, Uint256::from(2000000u128 / 2u128));

        // update creator 2 position at 3500
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(3500);
        let update_msg = crate::msg::ExecuteMsg::UpdatePosition {
            stream_id: 1,
            operator_target: None,
        };
        let info = mock_info("creator2", &[]);
        let _res = execute(deps.as_mut(), env.clone(), info, update_msg).unwrap();

        // query position
        let res = query_position(deps.as_ref(), env, 1, "creator2".to_string()).unwrap();
        assert_eq!(res.purchased, Uint256::from(115384u128));
        // spent =  in_supply * (now-last_updated) / (end-last_updated)
        assert_eq!(res.spent, Uint256::from(1000000u128 * 1500u128 / 4000u128));
        // query stream
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(3500);
        let stream = query_stream(deps.as_ref(), env, 1).unwrap();
        assert_eq!(stream.status, Status::Active);
        // in supply = 3000000 - (positions.spent summed)
        assert_eq!(stream.in_supply, Uint256::from(1625000u128));
    }

    #[test]
    pub fn test_withdraw_pending() {
        // // instantiate
        let treasury = Addr::unchecked("treasury");
        let start = Timestamp::from_seconds(2000);
        let end = Timestamp::from_seconds(1_000_000);
        let out_supply = Uint256::from(1_000_000u128);
        let out_denom = "out_denom";

        // instantiate
        let mut deps = mock_dependencies();
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(100);
        let msg = crate::msg::InstantiateMsg {
            min_stream_seconds: Uint64::new(1000),
            min_seconds_until_start_time: Uint64::new(1000),
            stream_creation_denom: "fee".to_string(),
            stream_creation_fee: Uint128::new(100),
            exit_fee_percent: Decimal256::percent(1),
            fee_collector: "collector".to_string(),
            protocol_admin: "protocol_admin".to_string(),
            accepted_in_denom: "in".to_string(),
        };
        instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

        // create stream
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(200);
        let info = mock_info(
            "creator1",
            &[
                Coin::new(out_supply.to_string().parse().unwrap(), out_denom),
                Coin::new(100, "fee"),
            ],
        );
        execute_create_stream(
            deps.as_mut(),
            env,
            info,
            treasury.to_string(),
            "test".to_string(),
            Some("https://sample.url".to_string()),
            "in".to_string(),
            out_denom.to_string(),
            out_supply,
            start,
            end,
            None,
        )
        .unwrap();

        // first subscribe before start time
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(300);
        let info = mock_info("creator1", &[Coin::new(1_000_000, "in")]);
        let msg = crate::msg::ExecuteMsg::Subscribe {
            stream_id: 1,
            operator_target: None,
            operator: None,
        };
        let _res = execute(deps.as_mut(), env, info, msg).unwrap();

        // update creator 1 position no distrubution is excepted
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(350);
        let update_msg = crate::msg::ExecuteMsg::UpdatePosition {
            stream_id: 1,
            operator_target: None,
        };
        let info = mock_info("creator1", &[]);
        let res = execute(deps.as_mut(), env, info, update_msg).unwrap();

        assert_eq!(res.attributes[1].key, "stream_id");
        assert_eq!(res.attributes[1].value, "1");
        assert_eq!(res.attributes[3].key, "purchased");
        assert_eq!(res.attributes[3].value, "0");
        assert_eq!(res.attributes[4].key, "spent");
        assert_eq!(res.attributes[4].value, "0");

        // query stream before withdraw
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(400);
        let stream = query_stream(deps.as_ref(), env, 1).unwrap();

        assert_eq!(stream.id, 1);
        assert_eq!(stream.dist_index, Decimal256::zero());
        assert_eq!(stream.last_updated, Timestamp::from_seconds(2000));
        assert_eq!(stream.in_supply, Uint256::from(1_000_000u128));
        assert_eq!(stream.spent_in, Uint256::zero());
        assert_eq!(stream.shares, Uint256::from(1_000_000u128));

        // withdraw before start time
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(400);
        let info = mock_info("creator1", &[]);
        let msg = crate::msg::ExecuteMsg::Withdraw {
            stream_id: 1,
            cap: Some(Uint256::from(500_000u128)),
            operator_target: None,
        };
        let res = execute(deps.as_mut(), env, info, msg).unwrap();
        assert_eq!(res.attributes[0].value, "withdraw_pending");
        assert_eq!(res.attributes[1].key, "stream_id");
        assert_eq!(res.attributes[1].value, "1");
        assert_eq!(res.attributes[3].key, "withdraw_amount");
        assert_eq!(res.attributes[3].value, "500000");
        assert_eq!(
            res.messages[0].msg,
            CosmosMsg::Bank(BankMsg::Send {
                to_address: "creator1".to_string(),
                amount: vec![Coin::new(500000, "in")]
            })
        );
        // query stream after withdraw
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(400);
        let stream = query_stream(deps.as_ref(), env, 1).unwrap();
        assert_eq!(stream.id, 1);
        assert_eq!(stream.dist_index, Decimal256::zero());
        assert_eq!(stream.last_updated, Timestamp::from_seconds(2000));
        assert_eq!(stream.in_supply, Uint256::from(500_000u128));
        assert_eq!(stream.spent_in, Uint256::zero());
        assert_eq!(stream.shares, Uint256::from(500_000u128));

        // withdraw after start time
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(3000);
        let info = mock_info("creator1", &[]);
        let msg = crate::msg::ExecuteMsg::Withdraw {
            stream_id: 1,
            cap: Some(Uint256::from(400_000u128)),
            operator_target: None,
        };
        let res = execute(deps.as_mut(), env, info, msg).unwrap();
        assert_eq!(res.attributes[0].value, "withdraw");
        assert_eq!(res.attributes[1].key, "stream_id");
        assert_eq!(res.attributes[1].value, "1");
        assert_eq!(res.attributes[3].key, "withdraw_amount");
        assert_eq!(res.attributes[3].value, "400000");
        assert_eq!(
            res.messages[0].msg,
            CosmosMsg::Bank(BankMsg::Send {
                to_address: "creator1".to_string(),
                amount: vec![Coin::new(400000, "in")]
            })
        );
        // query stream after withdraw
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(3000);
        let _stream = query_stream(deps.as_ref(), env, 1).unwrap();
    }

    #[test]
    fn test_operator() {
        let treasury = Addr::unchecked("treasury");
        let start = Timestamp::from_seconds(1_590_797_419);
        let end = Timestamp::from_seconds(5_571_797_419);
        let out_supply = Uint256::from(1_000_000u128);
        let out_denom = "out_denom";

        // instantiate
        let mut deps = mock_dependencies();
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(100);
        let msg = crate::msg::InstantiateMsg {
            min_stream_seconds: Uint64::new(1000),
            min_seconds_until_start_time: Uint64::new(1),
            stream_creation_denom: "fee".to_string(),
            stream_creation_fee: Uint128::new(100),
            exit_fee_percent: Decimal256::percent(1),
            fee_collector: "collector".to_string(),
            protocol_admin: "protocol_admin".to_string(),
            accepted_in_denom: "in".to_string(),
        };
        instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

        // create stream
        let env = mock_env();
        let info = mock_info(
            "creator",
            &[
                Coin::new(out_supply.to_string().parse().unwrap(), out_denom),
                Coin::new(100, "fee"),
            ],
        );
        execute_create_stream(
            deps.as_mut(),
            env,
            info,
            treasury.to_string(),
            "test".to_string(),
            Some("https://sample.url".to_string()),
            "in".to_string(),
            out_denom.to_string(),
            out_supply,
            start,
            end,
            None,
        )
        .unwrap();

        //random cannot make the first subscription on behalf of user
        let mut env = mock_env();
        let info = mock_info("random", &[Coin::new(1_000_000, "in")]);
        env.block.time = start.plus_seconds(100);
        let msg = crate::msg::ExecuteMsg::Subscribe {
            stream_id: 1,
            operator_target: Some("creator1".to_string()),
            operator: None,
        };
        let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(res, ContractError::Unauthorized {});

        //random cannot make the first subscription on behalf of user even if defined as operator in message
        let mut env = mock_env();
        let info = mock_info("random", &[Coin::new(1_000_000, "in")]);
        env.block.time = start.plus_seconds(100);
        let msg = crate::msg::ExecuteMsg::Subscribe {
            stream_id: 1,
            operator_target: Some("creator1".to_string()),
            operator: Some("random".to_string()),
        };
        let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(res, ContractError::Unauthorized {});

        // first subscription
        let mut env = mock_env();
        env.block.time = start.plus_seconds(100);
        let info = mock_info("creator1", &[Coin::new(1_000_000, "in")]);
        let msg = crate::msg::ExecuteMsg::Subscribe {
            stream_id: 1,
            operator_target: None,
            operator: None,
        };
        let _res = execute(deps.as_mut(), env, info, msg);

        // only owner can update
        let mut env = mock_env();
        let info = mock_info("creator2", &[]);
        env.block.time = start.plus_seconds(100);
        let res =
            execute_update_position(deps.as_mut(), env, info, 1, Some("creator1".to_string()))
                .unwrap_err();
        assert_eq!(res, ContractError::Unauthorized {});

        // owner can update with position owner field
        let info = mock_info("creator1", &[]);
        let mut env = mock_env();
        env.block.time = start.plus_seconds(100);
        let res =
            execute_update_position(deps.as_mut(), env, info, 1, Some("creator1".to_string()))
                .unwrap();
        assert_eq!(
            res,
            Response::new()
                .add_attribute("action", "update_position")
                .add_attribute("stream_id", "1")
                .add_attribute("operator_target", "creator1")
                .add_attribute("purchased", "0")
                .add_attribute("spent", "0")
        );

        // random cannot update
        let info = mock_info("random", &[]);
        let mut env = mock_env();
        env.block.time = start.plus_seconds(100);
        let res =
            execute_update_position(deps.as_mut(), env, info, 1, Some("creator1".to_string()))
                .unwrap_err();
        assert_eq!(res, ContractError::Unauthorized {});

        // random cannot withdraw
        let _info = mock_info("random", &[]);
        let mut env = mock_env();
        env.block.time = start.plus_seconds(100);
        let _msg = crate::msg::ExecuteMsg::Withdraw {
            stream_id: 1,
            cap: None,
            operator_target: Some("creator1".to_string()),
        };
        assert_eq!(res, ContractError::Unauthorized {});

        //owner can update operator
        let info = mock_info("creator1", &[]);
        let mut env = mock_env();
        let owner = "creator1".to_string();
        let stream_id = 1;
        env.block.time = start.plus_seconds(100);
        execute_update_operator(
            deps.as_mut(),
            env.clone(),
            info,
            1,
            Some("operator1".to_string()),
        )
        .unwrap();
        let position = query_position(deps.as_ref(), env, stream_id, owner).unwrap();
        assert_eq!(position.operator.unwrap().as_str(), "operator1".to_string());

        //operator can increase subscription on behalf of owner
        let info = mock_info("operator1", &[Coin::new(1_000_000, "in")]);
        let mut env = mock_env();
        env.block.time = start.plus_seconds(100);
        let msg = crate::msg::ExecuteMsg::Subscribe {
            stream_id: 1,
            operator_target: Some("creator1".to_string()),
            operator: None,
        };
        let res = execute(deps.as_mut(), env, info, msg).unwrap();
        assert_eq!(
            res,
            Response::new()
                .add_attribute("action", "subscribe")
                .add_attribute("stream_id", "1")
                .add_attribute("owner", "creator1")
                .add_attribute("in_supply", "2000000")
                .add_attribute("in_amount", "1000000")
        );

        // random cannot update operator
        let info = mock_info("random", &[]);
        let mut env = mock_env();
        env.block.time = start.plus_seconds(100);
        let res =
            execute_update_operator(deps.as_mut(), env, info, 1, Some("operator1".to_string()))
                .unwrap_err();
        assert!(matches!(res, ContractError::Std(StdError::NotFound { .. })));

        // operator can't update operator
        let info = mock_info("operator1", &[]);
        let mut env = mock_env();
        env.block.time = start.plus_seconds(100);
        let res =
            execute_update_operator(deps.as_mut(), env, info, 1, Some("operator2".to_string()))
                .unwrap_err();
        assert!(matches!(res, ContractError::Std(StdError::NotFound { .. })));

        // operator can update position
        let info = mock_info("operator1", &[]);
        let mut env = mock_env();
        env.block.time = start.plus_seconds(100);
        let res =
            execute_update_position(deps.as_mut(), env, info, 1, Some("creator1".to_string()))
                .unwrap();
        assert_eq!(
            res,
            Response::new()
                .add_attribute("action", "update_position")
                .add_attribute("stream_id", "1")
                .add_attribute("operator_target", "creator1")
                .add_attribute("purchased", "0")
                .add_attribute("spent", "0")
        );

        // operator can withdraw
        let _info = mock_info("operator1", &[]);
        let mut env = mock_env();
        env.block.time = start.plus_seconds(100);
        let _msg = crate::msg::ExecuteMsg::Withdraw {
            stream_id: 1,
            cap: Some(5u128.into()),
            operator_target: Some("creator1".to_string()),
        };

        // random cannot exit
        let info = mock_info("random", &[]);
        let mut env = mock_env();
        env.block.time = end.plus_seconds(100);
        execute_update_stream(deps.as_mut(), env.clone(), 1).unwrap();
        let res = execute_exit_stream(deps.as_mut(), env, info, 1, Some("creator1".to_string()))
            .unwrap_err();
        assert_eq!(res, ContractError::Unauthorized {});

        let mut env = mock_env();
        env.block.time = end.plus_seconds(100);
        execute_update_stream(deps.as_mut(), env, 1).unwrap();

        // operator can exit
        let info = mock_info("operator1", &[]);
        let mut env = mock_env();
        env.block.time = end.plus_seconds(100);
        let res =
            execute_exit_stream(deps.as_mut(), env, info, 1, Some("creator1".to_string())).unwrap();
        match res.messages.get(0).unwrap().msg.clone() {
            CosmosMsg::Bank(BankMsg::Send {
                to_address,
                amount: _,
            }) => {
                assert_eq!(to_address, "creator1");
            }
            _ => panic!("unexpected message"),
        }
    }

    #[test]
    fn test_update_stream() {
        let treasury = Addr::unchecked("treasury");
        let start = Timestamp::from_seconds(1_000_000);
        let end = Timestamp::from_seconds(5_000_000);
        let out_supply = Uint256::from(1_000_000u128);
        let out_denom = "out_denom";

        // instantiate
        let mut deps = mock_dependencies();
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(100);
        let msg = crate::msg::InstantiateMsg {
            min_stream_seconds: Uint64::new(1000),
            min_seconds_until_start_time: Uint64::new(1000),
            stream_creation_denom: "fee".to_string(),
            stream_creation_fee: Uint128::new(100),
            exit_fee_percent: Decimal256::percent(1),
            fee_collector: "collector".to_string(),
            protocol_admin: "protocol_admin".to_string(),
            accepted_in_denom: "in".to_string(),
        };
        instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

        // create stream
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(1);
        let info = mock_info(
            "creator",
            &[
                Coin::new(out_supply.to_string().parse().unwrap(), out_denom),
                Coin::new(100, "fee"),
            ],
        );
        execute_create_stream(
            deps.as_mut(),
            env,
            info,
            treasury.to_string(),
            "test".to_string(),
            Some("https://sample.url".to_string()),
            "in".to_string(),
            out_denom.to_string(),
            out_supply,
            start,
            end,
            None,
        )
        .unwrap();

        //update stream without subscription this means no new  distribution so returned index should be 0
        let mut env = mock_env();
        env.block.time = start.plus_seconds(100);
        let res = execute_update_stream(deps.as_mut(), env, 1).unwrap();
        assert_eq!(
            res,
            Response::default()
                .add_attribute("action", "update_stream")
                .add_attribute("stream_id", "1")
                .add_attribute("new_distribution_amount", "0")
                .add_attribute("dist_index", "0")
        );
        //first subscription
        //On first subscription index is not incresed because no distrubution prior to that(Execute_subscibe also includes update_stream)
        let mut env = mock_env();
        env.block.time = start.plus_seconds(100);
        let info = mock_info("creator1", &[Coin::new(1_000_000, "in")]);
        let msg = crate::msg::ExecuteMsg::Subscribe {
            stream_id: 1,
            operator_target: Some("creator1".to_string()),
            operator: None,
        };
        let _res = execute(deps.as_mut(), env, info, msg);

        //Query stream
        let mut env = mock_env();
        env.block.time = start.plus_seconds(200);
        let res = query_stream(deps.as_ref(), env, 1).unwrap();
        assert_eq!(res.dist_index, Decimal256::zero());

        //Update stream again, this time with subscriber
        let mut env = mock_env();
        env.block.time = start.plus_seconds(300);
        execute_update_stream(deps.as_mut(), env, 1).unwrap();

        //Query stream
        let mut env = mock_env();
        env.block.time = start.plus_seconds(300);
        let res = query_stream(deps.as_ref(), env, 1).unwrap();
        assert_eq!(res.dist_index, Decimal256::from_str("0.00005").unwrap())
    }
    #[test]
    fn test_update_position() {
        let treasury = Addr::unchecked("treasury");
        let start = Timestamp::from_seconds(1_000_000);
        let end = Timestamp::from_seconds(5_000_000);
        let out_supply = Uint256::from(1_000_000u128);
        let out_denom = "out_denom";

        // instantiate
        let mut deps = mock_dependencies();
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(100);
        let msg = crate::msg::InstantiateMsg {
            min_stream_seconds: Uint64::new(1000),
            min_seconds_until_start_time: Uint64::new(1000),
            stream_creation_denom: "fee".to_string(),
            stream_creation_fee: Uint128::new(100),
            exit_fee_percent: Decimal256::percent(1),
            fee_collector: "collector".to_string(),
            protocol_admin: "protocol_admin".to_string(),
            accepted_in_denom: "in".to_string(),
        };
        instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

        // create stream
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(1);
        let info = mock_info(
            "creator",
            &[
                Coin::new(out_supply.to_string().parse().unwrap(), out_denom),
                Coin::new(100, "fee"),
            ],
        );
        execute_create_stream(
            deps.as_mut(),
            env,
            info,
            treasury.to_string(),
            "test".to_string(),
            Some("https://sample.url".to_string()),
            "in".to_string(),
            out_denom.to_string(),
            out_supply,
            start,
            end,
            None,
        )
        .unwrap();

        // first subscription
        let mut env = mock_env();
        env.block.time = start.plus_seconds(100);
        let info = mock_info("creator1", &[Coin::new(1_000_000, "in")]);
        let msg = crate::msg::ExecuteMsg::Subscribe {
            stream_id: 1,
            operator_target: None,
            operator: None,
        };
        let _res = execute(deps.as_mut(), env, info, msg);

        // non owner operator cannot update position
        let mut env = mock_env();
        env.block.time = start.plus_seconds(3_000_000);
        let info = mock_info("random", &[]);
        let err =
            execute_update_position(deps.as_mut(), env, info, 1, Some("creator1".to_string()))
                .unwrap_err();
        assert_eq!(err, ContractError::Unauthorized {});

        // update position
        let mut env = mock_env();
        env.block.time = start.plus_seconds(3_000_000);
        let info = mock_info("creator1", &[]);
        execute_update_position(deps.as_mut(), env.clone(), info, 1, None).unwrap();

        let position =
            query_position(deps.as_ref(), env.clone(), 1, "creator1".to_string()).unwrap();
        assert_eq!(
            position.index,
            Decimal256::from_str("0.749993000000000000").unwrap()
        );
        assert_eq!(position.purchased, Uint256::from(749_993u128));
        assert_eq!(position.spent, Uint256::from(749_993u128));
        assert_eq!(position.in_balance, Uint256::from(250_007u128));
        let stream = query_stream(deps.as_ref(), env, 1).unwrap();
        assert_eq!(
            stream.dist_index,
            Decimal256::from_str("0.749993000000000000").unwrap()
        );

        // can update position after stream ends
        let mut env = mock_env();
        env.block.time = end.plus_seconds(1);
        let info = mock_info("creator1", &[]);
        execute_update_position(deps.as_mut(), env.clone(), info, 1, None).unwrap();
        let stream = query_stream(deps.as_ref(), env.clone(), 1).unwrap();
        assert_eq!(stream.dist_index, Decimal256::from_str("1").unwrap());
        assert_eq!(stream.in_supply, Uint256::zero());
        let position = query_position(deps.as_ref(), env, 1, "creator1".to_string()).unwrap();
        assert_eq!(position.index, Decimal256::from_str("1").unwrap());
        assert_eq!(position.spent, Uint256::from(1_000_000u128));
        assert_eq!(position.in_balance, Uint256::zero());

        assert_eq!(stream.out_supply, Uint256::from(1_000_000u128));
        assert_eq!(position.purchased, stream.out_supply);
    }

    // this is for testing the leftover amount with bigger values
    #[test]
    fn test_rounding_leftover() {
        let treasury = Addr::unchecked("treasury");
        let start = Timestamp::from_seconds(1_000_000);
        let end = Timestamp::from_seconds(5_000_000);
        let out_supply = Uint256::from(1_000_000_000_000u128);
        let out_denom = "out_denom";

        // instantiate
        let mut deps = mock_dependencies();
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(100);
        let msg = crate::msg::InstantiateMsg {
            min_stream_seconds: Uint64::new(1000),
            min_seconds_until_start_time: Uint64::new(1000),
            stream_creation_denom: "fee".to_string(),
            stream_creation_fee: Uint128::new(100),
            exit_fee_percent: Decimal256::percent(1),
            fee_collector: "collector".to_string(),
            protocol_admin: "protocol_admin".to_string(),
            accepted_in_denom: "in".to_string(),
        };
        instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

        // create stream
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(1);
        let info = mock_info(
            "creator",
            &[
                Coin::new(out_supply.to_string().parse().unwrap(), out_denom),
                Coin::new(100, "fee"),
            ],
        );
        execute_create_stream(
            deps.as_mut(),
            env,
            info,
            treasury.to_string(),
            "test".to_string(),
            Some("https://sample.url".to_string()),
            "in".to_string(),
            out_denom.to_string(),
            out_supply,
            start,
            end,
            None,
        )
        .unwrap();

        // first subscription
        let mut env = mock_env();
        env.block.time = start.plus_seconds(100);
        let info = mock_info("creator1", &[Coin::new(1_000_000_000, "in")]);
        let msg = crate::msg::ExecuteMsg::Subscribe {
            stream_id: 1,
            operator_target: None,
            operator: None,
        };
        execute(deps.as_mut(), env, info, msg).unwrap();

        // second subscription
        let mut env = mock_env();
        env.block.time = start.plus_seconds(100_000);
        let info = mock_info("creator2", &[Coin::new(3_000_000_000, "in")]);
        let msg = crate::msg::ExecuteMsg::Subscribe {
            stream_id: 1,
            operator_target: None,
            operator: None,
        };
        execute(deps.as_mut(), env, info, msg).unwrap();

        // update position creator1
        let mut env = mock_env();
        env.block.time = start.plus_seconds(3_000_000);
        let info = mock_info("creator1", &[]);
        execute_update_position(deps.as_mut(), env.clone(), info, 1, None).unwrap();

        let position =
            query_position(deps.as_ref(), env.clone(), 1, "creator1".to_string()).unwrap();
        assert_eq!(
            position.index,
            Decimal256::from_str("202.813614449380587585").unwrap()
        );
        assert_eq!(position.purchased, Uint256::from(202_813_614_449u128));
        assert_eq!(position.spent, Uint256::from(749_993_750u128));
        assert_eq!(position.in_balance, Uint256::from(250_006_250u128));
        let stream = query_stream(deps.as_ref(), env, 1).unwrap();
        assert_eq!(
            stream.dist_index,
            Decimal256::from_str("202.813614449380587585").unwrap()
        );

        // update position creator2
        let mut env = mock_env();
        env.block.time = start.plus_seconds(3_575_000);
        let info = mock_info("creator2", &[]);
        execute_update_position(deps.as_mut(), env.clone(), info, 1, None).unwrap();

        let position =
            query_position(deps.as_ref(), env.clone(), 1, "creator2".to_string()).unwrap();
        assert_eq!(
            position.index,
            Decimal256::from_str("238.074595237060799266").unwrap()
        );
        assert_eq!(position.purchased, Uint256::from(655672748445u128));
        assert_eq!(position.spent, Uint256::from(2673076923u128));
        assert_eq!(position.in_balance, Uint256::from(326923077u128));
        let stream = query_stream(deps.as_ref(), env, 1).unwrap();
        assert_eq!(
            stream.dist_index,
            Decimal256::from_str("238.074595237060799266").unwrap()
        );

        // update position after stream ends
        let mut env = mock_env();
        env.block.time = end.plus_seconds(1);
        let info = mock_info("creator1", &[]);
        execute_update_position(deps.as_mut(), env.clone(), info, 1, None).unwrap();
        let stream = query_stream(deps.as_ref(), env.clone(), 1).unwrap();
        assert_eq!(
            stream.dist_index,
            Decimal256::from_str("264.137059297637397644").unwrap()
        );
        assert_eq!(stream.in_supply, Uint256::zero());
        let position1 = query_position(deps.as_ref(), env, 1, "creator1".to_string()).unwrap();
        assert_eq!(
            position1.index,
            Decimal256::from_str("264.137059297637397644").unwrap()
        );
        assert_eq!(position1.spent, Uint256::from(1_000_000_000u128));
        assert_eq!(position1.in_balance, Uint256::zero());

        // update position after stream ends
        let mut env = mock_env();
        env.block.time = end.plus_seconds(1);
        let info = mock_info("creator2", &[]);
        execute_update_position(deps.as_mut(), env.clone(), info, 1, None).unwrap();
        let stream = query_stream(deps.as_ref(), env.clone(), 1).unwrap();
        assert_eq!(
            stream.dist_index,
            Decimal256::from_str("264.137059297637397644").unwrap()
        );
        assert_eq!(stream.in_supply, Uint256::zero());
        let position2 = query_position(deps.as_ref(), env, 1, "creator2".to_string()).unwrap();
        assert_eq!(
            position2.index,
            Decimal256::from_str("264.137059297637397644").unwrap()
        );
        assert_eq!(position2.spent, Uint256::from(3_000_000_000u128));
        assert_eq!(position2.in_balance, Uint256::zero());

        assert_eq!(stream.out_remaining, Uint256::zero());
        assert_eq!(
            position1
                .purchased
                .checked_add(position2.purchased)
                .unwrap(),
            // 1 difference due to rounding
            stream.out_supply.sub(Uint256::from(1u128))
        );
    }

    #[test]
    fn test_withdraw() {
        let treasury = Addr::unchecked("treasury");
        let start = Timestamp::from_seconds(1_000_000);
        let end = Timestamp::from_seconds(5_000_000);
        let out_supply = Uint256::from(1_000_000_000_000u128);
        let out_denom = "out_denom";

        // instantiate
        let mut deps = mock_dependencies();
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(0);
        let msg = crate::msg::InstantiateMsg {
            min_stream_seconds: Uint64::new(1000),
            min_seconds_until_start_time: Uint64::new(0),
            stream_creation_denom: "fee".to_string(),
            stream_creation_fee: Uint128::new(100),
            exit_fee_percent: Decimal256::percent(1),
            fee_collector: "collector".to_string(),
            protocol_admin: "protocol_admin".to_string(),
            accepted_in_denom: "in".to_string(),
        };
        instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

        // create stream
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(0);
        let info = mock_info(
            "creator1",
            &[
                Coin::new(out_supply.to_string().parse().unwrap(), out_denom),
                Coin::new(100, "fee"),
            ],
        );
        execute_create_stream(
            deps.as_mut(),
            env,
            info,
            treasury.to_string(),
            "test".to_string(),
            Some("https://sample.url".to_string()),
            "in".to_string(),
            out_denom.to_string(),
            out_supply,
            start,
            end,
            None,
        )
        .unwrap();

        // first subscription
        let mut env = mock_env();
        env.block.time = start.plus_seconds(0);
        let funds = Coin::new(2_000_000_000_000, "in");
        let info = mock_info("creator1", &[funds.clone()]);
        let msg = crate::msg::ExecuteMsg::Subscribe {
            stream_id: 1,
            operator_target: None,
            operator: None,
        };
        let _res = execute(deps.as_mut(), env, info, msg).unwrap();

        // withdraw with cap
        let mut env = mock_env();
        env.block.time = start.plus_seconds(5000);
        let info = mock_info("creator1", &[]);
        // withdraw amount zero
        let cap = Uint256::zero();
        let msg = crate::msg::ExecuteMsg::Withdraw {
            stream_id: 1,
            cap: Some(cap),
            operator_target: None,
        };
        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap_err();
        assert_eq!(res, ContractError::InvalidWithdrawAmount {});
        // withdraw amount too high
        let cap = Uint256::from(2_250_000_000_000u128);
        let msg = crate::msg::ExecuteMsg::Withdraw {
            stream_id: 1,
            cap: Some(cap),
            operator_target: None,
        };
        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap_err();
        assert_eq!(
            res,
            ContractError::WithdrawAmountExceedsBalance(Uint256::from(2250000000000u128))
        );
        //withdraw with valid cap
        let cap = Uint256::from(25_000_000u128);
        let msg = crate::msg::ExecuteMsg::Withdraw {
            stream_id: 1,
            cap: Some(cap),
            operator_target: None,
        };
        let _res = execute(deps.as_mut(), env, info, msg).unwrap();
        let position =
            query_position(deps.as_ref(), mock_env(), 1, "creator1".to_string()).unwrap();
        assert_eq!(position.in_balance, Uint256::from(1_997_475_000_000u128));
        assert_eq!(position.spent, Uint256::from(2_500_000_000u128));
        assert_eq!(position.purchased, Uint256::from(1_250_000_000u128));
        // first fund amount should be equal to in_balance + spent + cap
        assert_eq!(
            position.in_balance + position.spent + cap,
            Uint256::from_str(funds.amount.to_string().as_str()).unwrap()
        );

        let mut env = mock_env();
        env.block.time = start.plus_seconds(1_000_000);
        let info = mock_info("creator1", &[]);
        let msg = crate::msg::ExecuteMsg::Withdraw {
            stream_id: 1,
            cap: None,
            operator_target: None,
        };
        let res = execute(deps.as_mut(), env, info, msg).unwrap();
        let position =
            query_position(deps.as_ref(), mock_env(), 1, "creator1".to_string()).unwrap();
        assert_eq!(position.in_balance, Uint256::zero());
        assert_eq!(position.spent, Uint256::from(499_993_773_466u128));
        assert_eq!(position.purchased, Uint256::from(249_999_999_998u128));
        assert_eq!(position.shares, Uint256::zero());
        let msg = res.messages.get(0).unwrap();
        assert_eq!(
            msg.msg,
            CosmosMsg::Bank(BankMsg::Send {
                to_address: "creator1".to_string(),
                amount: vec![Coin::new(1_499_981_226_534, "in")]
            })
        );

        // can't withdraw after stream ends
        let mut env = mock_env();
        env.block.time = end.plus_seconds(1);
        let info = mock_info("creator1", &[]);
        let msg = crate::msg::ExecuteMsg::Withdraw {
            stream_id: 1,
            cap: None,
            operator_target: None,
        };
        let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(res, ContractError::StreamEnded {});
    }

    #[test]
    fn test_finalize_stream() {
        let treasury = Addr::unchecked("treasury");
        let start = Timestamp::from_seconds(1_000_000);
        let end = Timestamp::from_seconds(5_000_000);
        let out_supply = Uint256::from(1_000_000_000_000u128);
        let out_denom = "out_denom";

        // instantiate
        let mut deps = mock_dependencies();
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(0);
        let msg = crate::msg::InstantiateMsg {
            min_stream_seconds: Uint64::new(1000),
            min_seconds_until_start_time: Uint64::new(0),
            stream_creation_denom: "fee".to_string(),
            stream_creation_fee: Uint128::new(100),
            exit_fee_percent: Decimal256::percent(1),
            fee_collector: "collector".to_string(),
            protocol_admin: "protocol_admin".to_string(),
            accepted_in_denom: "in".to_string(),
        };
        instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

        // create stream
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(0);
        let info = mock_info(
            "creator1",
            &[
                Coin::new(out_supply.to_string().parse().unwrap(), out_denom),
                Coin::new(100, "fee"),
            ],
        );
        execute_create_stream(
            deps.as_mut(),
            env,
            info,
            treasury.to_string(),
            "test".to_string(),
            Some("https://sample.url".to_string()),
            "in".to_string(),
            out_denom.to_string(),
            out_supply,
            start,
            end,
            None,
        )
        .unwrap();

        // first subscription
        let mut env = mock_env();
        env.block.time = start.plus_seconds(1_000_000);
        let funds = Coin::new(2_000_000_000_000, "in");
        let info = mock_info("creator1", &[funds]);
        let msg = crate::msg::ExecuteMsg::Subscribe {
            stream_id: 1,
            operator_target: None,
            operator: None,
        };
        let _res = execute(deps.as_mut(), env, info, msg).unwrap();

        // only treasury can finalize
        let mut env = mock_env();
        env.block.time = end.plus_seconds(1);
        let info = mock_info("random", &[]);
        let res = execute_finalize_stream(deps.as_mut(), env, info, 1, None).unwrap_err();
        assert_eq!(res, ContractError::Unauthorized {});

        // can't finalize before stream ends
        let mut env = mock_env();
        env.block.time = start.plus_seconds(1);
        let info = mock_info(treasury.as_str(), &[]);
        let res = execute_finalize_stream(deps.as_mut(), env, info, 1, None).unwrap_err();
        assert_eq!(res, ContractError::StreamNotEnded {});

        // happy path
        let mut env = mock_env();
        env.block.time = end.plus_seconds(1);
        let info = mock_info(treasury.as_str(), &[]);
        execute_update_stream(deps.as_mut(), env.clone(), 1).unwrap();

        let res = execute_finalize_stream(deps.as_mut(), env, info, 1, None).unwrap();
        assert_eq!(
            res.attributes,
            vec![
                attr("action", "finalize_stream"),
                attr("stream_id", "1"),
                attr("treasury", "treasury"),
                attr("fee_collector", "collector"),
                attr("creators_revenue", "1980000000000"),
                attr("refunded_out_remaining", "0"),
                attr("total_sold", "1000000000000"),
                attr("swap_fee", "20000000000"),
                attr("creation_fee", "100"),
            ]
        );
        assert_eq!(
            res.messages,
            vec![
                SubMsg::new(BankMsg::Send {
                    to_address: "treasury".to_string(),
                    amount: vec![Coin {
                        denom: "in".to_string(),
                        amount: Uint128::new(1_980_000_000_000),
                    }],
                }),
                SubMsg::new(BankMsg::Send {
                    to_address: "collector".to_string(),
                    amount: vec![Coin {
                        denom: "fee".to_string(),
                        amount: Uint128::new(100),
                    }],
                }),
                SubMsg::new(BankMsg::Send {
                    to_address: "collector".to_string(),
                    amount: vec![Coin {
                        denom: "in".to_string(),
                        amount: Uint128::new(20_000_000_000),
                    }],
                }),
            ],
        );
    }

    #[test]
    fn test_recurring_finalize_stream_calls() {
        let malicious_treasury = Addr::unchecked("treasury");
        let start = Timestamp::from_seconds(10);
        let end = Timestamp::from_seconds(110);
        let out_supply = Uint256::from(1000u128);
        let out_denom = "myToken";
        let in_denom = "uosmo";
        // instantiate
        let mut deps = mock_dependencies();
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(0);
        let msg = crate::msg::InstantiateMsg {
            min_stream_seconds: Uint64::new(100),
            min_seconds_until_start_time: Uint64::new(0),
            stream_creation_denom: "fee".to_string(),
            stream_creation_fee: Uint128::new(100),
            exit_fee_percent: Decimal256::percent(1),
            fee_collector: "collector".to_string(),
            protocol_admin: "protocol_admin".to_string(),
            accepted_in_denom: in_denom.to_string(),
        };
        instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();
        // Create stream
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(0);
        let info = mock_info(
            malicious_treasury.as_str(),
            &[
                Coin::new(out_supply.to_string().parse().unwrap(), out_denom),
                Coin::new(100, "fee"),
            ],
        );
        execute_create_stream(
            deps.as_mut(),
            env,
            info,
            malicious_treasury.to_string(),
            "test".to_string(),
            Some("https://sample.url".to_string()),
            in_denom.to_string(),
            out_denom.to_string(),
            out_supply,
            start,
            end,
            None,
        )
        .unwrap();
        // First subscription
        let mut env = mock_env();
        env.block.time = start.plus_seconds(1);
        let funds = Coin::new(200, in_denom.to_string());
        let info = mock_info("user1", &[funds]);
        let msg = crate::msg::ExecuteMsg::Subscribe {
            stream_id: 1,
            operator_target: None,
            operator: None,
        };
        let _res = execute(deps.as_mut(), env, info, msg).unwrap();
        // Update
        let mut env = mock_env();
        env.block.time = end.plus_seconds(1);
        let info = mock_info(malicious_treasury.as_str(), &[]);
        execute_update_stream(deps.as_mut(), env.clone(), 1).unwrap();
        // First call
        let res =
            execute_finalize_stream(deps.as_mut(), env.clone(), info.clone(), 1, None).unwrap();
        assert_eq!(
            res.messages,
            vec![
                SubMsg::new(BankMsg::Send {
                    to_address: malicious_treasury.to_string(),
                    amount: vec![Coin {
                        denom: in_denom.to_string(),
                        amount: Uint128::new(198),
                    }],
                }),
                SubMsg::new(BankMsg::Send {
                    to_address: "collector".to_string(),
                    amount: vec![Coin {
                        denom: "fee".to_string(),
                        amount: Uint128::new(100),
                    }],
                }),
                SubMsg::new(BankMsg::Send {
                    to_address: "collector".to_string(),
                    amount: vec![Coin {
                        denom: in_denom.to_string(),
                        amount: Uint128::new(2),
                    }],
                }),
            ],
        );
        // Check stream status
        let stream = query_stream(deps.as_ref(), env.clone(), 1).unwrap();
        assert_eq!(stream.status, Status::Finalized);
        // Sequential calls, anyone could force this sequential calls
        let res = execute_finalize_stream(deps.as_mut(), env, info, 1, None).unwrap_err();
        assert_eq!(res, ContractError::StreamAlreadyFinalized {});
    }

    #[test]
    fn test_exit_stream() {
        let treasury = Addr::unchecked("treasury");
        let start = Timestamp::from_seconds(1_000_000);
        let end = Timestamp::from_seconds(5_000_000);
        let out_supply = Uint256::from(1_000_000_000_000u128);
        let out_denom = "out_denom";

        // instantiate
        let mut deps = mock_dependencies();
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(0);
        let msg = crate::msg::InstantiateMsg {
            min_stream_seconds: Uint64::new(1000),
            min_seconds_until_start_time: Uint64::new(0),
            stream_creation_denom: "fee".to_string(),
            stream_creation_fee: Uint128::new(100),
            exit_fee_percent: Decimal256::percent(1),
            fee_collector: "collector".to_string(),
            protocol_admin: "protocol_admin".to_string(),
            accepted_in_denom: "in".to_string(),
        };
        instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

        // create stream
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(0);
        let info = mock_info(
            "creator1",
            &[
                Coin::new(out_supply.to_string().parse().unwrap(), out_denom),
                Coin::new(100, "fee"),
            ],
        );
        execute_create_stream(
            deps.as_mut(),
            env,
            info,
            treasury.to_string(),
            "test".to_string(),
            Some("https://sample.url".to_string()),
            "in".to_string(),
            out_denom.to_string(),
            out_supply,
            start,
            end,
            None,
        )
        .unwrap();

        // first subscription
        let mut env = mock_env();
        env.block.time = start.plus_seconds(1_000_000);
        let funds = Coin::new(2_000_000_000_000, "in");
        let info = mock_info("creator1", &[funds]);
        let msg = crate::msg::ExecuteMsg::Subscribe {
            stream_id: 1,
            operator_target: None,
            operator: None,
        };
        let _res = execute(deps.as_mut(), env, info, msg).unwrap();

        // can't exit before stream ends
        let mut env = mock_env();
        env.block.time = start.plus_seconds(2_000_000);
        let info = mock_info("creator1", &[]);
        let res = execute_exit_stream(deps.as_mut(), env, info, 1, None).unwrap_err();
        assert_eq!(res, ContractError::StreamNotEnded {});

        //failed exit from random address
        let mut env = mock_env();
        env.block.time = end.plus_seconds(3_000_000);
        let info = mock_info("random", &[]);
        let res = execute_exit_stream(
            deps.as_mut(),
            env.clone(),
            info,
            1,
            Some("creator1".to_string()),
        )
        .unwrap_err();
        assert_eq!(res, ContractError::Unauthorized {});
        // can exit
        let info = mock_info("creator1", &[]);
        let res = execute_exit_stream(deps.as_mut(), env, info, 1, None).unwrap();
        assert_eq!(
            res.messages,
            vec![SubMsg::new(CosmosMsg::Bank(BankMsg::Send {
                to_address: "creator1".to_string(),
                amount: vec![Coin::new(
                    Uint128::new(1_000_000_000_000).u128(),
                    "out_denom"
                )]
            }))]
        );

        // position deleted
        let mut env = mock_env();
        env.block.time = end.plus_seconds(4_000_000);
        let info = mock_info("creator1", &[]);
        let res = execute_exit_stream(deps.as_mut(), env, info, 1, None).unwrap_err();
        assert!(matches!(res, ContractError::Std(StdError::NotFound { .. })));
    }

    #[test]
    fn test_withdraw_all_before_exit_case() {
        let treasury = Addr::unchecked("treasury");
        let start = Timestamp::from_seconds(1_000_000);
        let end = Timestamp::from_seconds(5_000_000);
        let out_supply = Uint256::from(1_000_000_000_000u128);
        let out_denom = "out_denom";

        // instantiate
        let mut deps = mock_dependencies();
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(0);
        let msg = crate::msg::InstantiateMsg {
            min_stream_seconds: Uint64::new(1000),
            min_seconds_until_start_time: Uint64::new(0),
            stream_creation_denom: "fee".to_string(),
            stream_creation_fee: Uint128::new(100),
            exit_fee_percent: Decimal256::percent(1),
            fee_collector: "collector".to_string(),
            protocol_admin: "protocol_admin".to_string(),
            accepted_in_denom: "in".to_string(),
        };
        instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

        // create stream
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(0);
        let info = mock_info(
            "creator1",
            &[
                Coin::new(out_supply.to_string().parse().unwrap(), out_denom),
                Coin::new(100, "fee"),
            ],
        );
        execute_create_stream(
            deps.as_mut(),
            env,
            info,
            treasury.to_string(),
            "test".to_string(),
            Some("https://sample.url".to_string()),
            "in".to_string(),
            out_denom.to_string(),
            out_supply,
            start,
            end,
            None,
        )
        .unwrap();

        // first subscription
        let mut env = mock_env();
        env.block.time = start.plus_seconds(1_000_000);
        let funds = Coin::new(2_000_000_000_000, "in");
        let info = mock_info("creator1", &[funds]);
        let msg = crate::msg::ExecuteMsg::Subscribe {
            stream_id: 1,
            operator_target: None,
            operator: None,
        };
        let _res = execute(deps.as_mut(), env, info, msg).unwrap();

        // second subscription
        let mut env = mock_env();
        env.block.time = start.plus_seconds(1_000_000);
        let funds = Coin::new(1_000_000_000_000, "in");
        let info = mock_info("creator2", &[funds]);
        let msg = crate::msg::ExecuteMsg::Subscribe {
            stream_id: 1,
            operator_target: None,
            operator: None,
        };
        let _res = execute(deps.as_mut(), env, info, msg).unwrap();

        // first withdraw
        let info = mock_info("creator1", &[]);
        let mut env = mock_env();
        env.block.time = end.minus_seconds(1_000_000);
        let msg = crate::msg::ExecuteMsg::Withdraw {
            stream_id: 1,
            cap: None,
            operator_target: None,
        };
        let _res = execute(deps.as_mut(), env, info, msg).unwrap();

        // second withdraw
        let info = mock_info("creator2", &[]);
        let mut env = mock_env();
        env.block.time = end.minus_seconds(1_000_000);
        let msg = crate::msg::ExecuteMsg::Withdraw {
            stream_id: 1,
            cap: None,
            operator_target: None,
        };
        let _res = execute(deps.as_mut(), env, info, msg).unwrap();

        // can exit
        let mut env = mock_env();
        env.block.time = end.plus_seconds(1_000_000);
        execute_update_stream(deps.as_mut(), env, 1).unwrap();

        let mut env = mock_env();
        env.block.time = end.plus_seconds(1_000_001);
        let info = mock_info("creator1", &[]);
        execute_exit_stream(deps.as_mut(), env, info, 1, None).unwrap();

        let mut env = mock_env();
        env.block.time = end.plus_seconds(1_000_002);
        let info = mock_info("creator2", &[]);
        execute_exit_stream(deps.as_mut(), env, info, 1, None).unwrap();
    }

    #[test]
    fn test_price_feed() {
        let treasury = Addr::unchecked("treasury");
        let start = Timestamp::from_seconds(1_000_000);
        let end = Timestamp::from_seconds(5_000_000);
        let out_supply = Uint256::from(1_000_000u128);
        let out_denom = "out_denom";

        // instantiate
        let mut deps = mock_dependencies();
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(0);
        let msg = crate::msg::InstantiateMsg {
            min_stream_seconds: Uint64::new(1000),
            min_seconds_until_start_time: Uint64::new(0),
            stream_creation_denom: "fee".to_string(),
            stream_creation_fee: Uint128::new(100),
            exit_fee_percent: Decimal256::percent(1),
            fee_collector: "collector".to_string(),
            protocol_admin: "protocol_admin".to_string(),
            accepted_in_denom: "in".to_string(),
        };
        instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

        // create stream
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(0);
        let info = mock_info(
            "creator1",
            &[
                Coin::new(out_supply.to_string().parse().unwrap(), out_denom),
                Coin::new(100, "fee"),
            ],
        );
        execute_create_stream(
            deps.as_mut(),
            env,
            info,
            treasury.to_string(),
            "test".to_string(),
            Some("https://sample.url".to_string()),
            "in".to_string(),
            out_denom.to_string(),
            out_supply,
            start,
            end,
            None,
        )
        .unwrap();

        // first subscription
        let mut env = mock_env();
        env.block.time = start.plus_seconds(1_000_000);
        let funds = Coin::new(3_000, "in");
        let info = mock_info("creator1", &[funds]);
        let msg = crate::msg::ExecuteMsg::Subscribe {
            stream_id: 1,
            operator_target: None,
            operator: None,
        };
        let _res = execute(deps.as_mut(), env, info, msg).unwrap();

        //check current streamed price before update
        let mut env = mock_env();
        env.block.time = start.plus_seconds(2_000_000);
        let res = query_last_streamed_price(deps.as_ref(), env, 1).unwrap();
        assert_eq!(res.current_streamed_price, Decimal256::zero());

        //check current streamed price after update
        let mut env = mock_env();
        env.block.time = start.plus_seconds(2_000_000);
        execute_update_stream(deps.as_mut(), env, 1).unwrap();
        let res = query_last_streamed_price(deps.as_ref(), mock_env(), 1).unwrap();
        //approx 1000/333333
        assert_eq!(
            res.current_streamed_price,
            Decimal256::from_str("0.002997002997002997").unwrap()
        );
        // second subscription
        let mut env = mock_env();
        env.block.time = start.plus_seconds(2_000_000);
        let funds = Coin::new(1_000, "in");
        let info = mock_info("creator2", &[funds]);
        let msg = crate::msg::ExecuteMsg::Subscribe {
            stream_id: 1,
            operator_target: None,
            operator: None,
        };
        let _res = execute(deps.as_mut(), env, info, msg).unwrap();

        //check current streamed price before update
        let mut env = mock_env();
        env.block.time = start.plus_seconds(3_000_000);
        let res = query_last_streamed_price(deps.as_ref(), env, 1).unwrap();
        assert_eq!(
            res.current_streamed_price,
            Decimal256::from_str("0.002997002997002997").unwrap()
        );

        //check current streamed price after update
        let mut env = mock_env();
        env.block.time = start.plus_seconds(3_000_000);
        execute_update_stream(deps.as_mut(), env, 1).unwrap();
        let res = query_last_streamed_price(deps.as_ref(), mock_env(), 1).unwrap();
        //approx 2000/333333
        assert_eq!(
            res.current_streamed_price,
            Decimal256::from_str("0.0045000045000045").unwrap()
        );

        //check average streamed price
        let mut env = mock_env();
        env.block.time = start.plus_seconds(3_000_000);
        let res = query_average_price(deps.as_ref(), env, 1).unwrap();
        //approx 2500/333333
        assert_eq!(
            res.average_price,
            Decimal256::from_str("0.003748503748503748").unwrap()
        );

        //withdraw creator 1
        let info = mock_info("creator1", &[]);
        let mut env = mock_env();
        env.block.time = start.plus_seconds(3_500_000);
        let msg = crate::msg::ExecuteMsg::Withdraw {
            stream_id: 1,
            cap: None,
            operator_target: None,
        };
        let _res = execute(deps.as_mut(), env, info, msg).unwrap();
        let res = query_last_streamed_price(deps.as_ref(), mock_env(), 1).unwrap();
        assert_eq!(
            res.current_streamed_price,
            Decimal256::from_str("0.004499991000017999").unwrap()
        );

        //test price after withdraw
        let mut env = mock_env();
        env.block.time = start.plus_seconds(3_750_000);
        execute_update_stream(deps.as_mut(), env, 1).unwrap();
        let res = query_last_streamed_price(deps.as_ref(), mock_env(), 1).unwrap();
        //approx 2500/333333
        assert_eq!(
            res.current_streamed_price,
            Decimal256::from_str("0.001500006000024000").unwrap()
        );
    }

    #[test]
    fn test_update_protocol_admin() {
        // instantiate
        let mut deps = mock_dependencies();
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(0);
        let msg = crate::msg::InstantiateMsg {
            min_stream_seconds: Uint64::new(1000),
            min_seconds_until_start_time: Uint64::new(0),
            stream_creation_denom: "fee".to_string(),
            stream_creation_fee: Uint128::new(100),
            exit_fee_percent: Decimal256::percent(1),
            fee_collector: "collector".to_string(),
            protocol_admin: "protocol_admin".to_string(),
            accepted_in_denom: "in".to_string(),
        };
        instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

        // random cannot update
        let env = mock_env();
        let msg = UpdateProtocolAdmin {
            new_protocol_admin: "new_protocol_admin".to_string(),
        };
        let info = mock_info("random", &[]);
        let err = execute(deps.as_mut(), env.clone(), info, msg.clone()).unwrap_err();
        assert_eq!(err, ContractError::Unauthorized {});

        // protocol admin can update
        let info = mock_info("protocol_admin", &[]);
        execute(deps.as_mut(), env, info, msg).unwrap();
        let query = query_config(deps.as_ref()).unwrap();
        assert_eq!(query.protocol_admin, "new_protocol_admin".to_string());
    }
    #[test]
    fn test_execute_update_config() {
        // instantiate
        let mut deps = mock_dependencies();
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(0);
        let msg = crate::msg::InstantiateMsg {
            min_stream_seconds: Uint64::new(1000),
            min_seconds_until_start_time: Uint64::new(0),
            stream_creation_denom: "fee".to_string(),
            stream_creation_fee: Uint128::new(100),
            exit_fee_percent: Decimal256::percent(1),
            fee_collector: "collector".to_string(),
            protocol_admin: "protocol_admin".to_string(),
            accepted_in_denom: "in".to_string(),
        };
        instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

        //query config
        let config_response = query_config(deps.as_ref()).unwrap();
        //check config
        assert_eq!(config_response.min_stream_seconds, Uint64::new(1000));
        assert_eq!(config_response.min_seconds_until_start_time, Uint64::new(0));
        assert_eq!(config_response.stream_creation_denom, "fee".to_string());
        assert_eq!(config_response.stream_creation_fee, Uint128::new(100));
        assert_eq!(config_response.fee_collector, "collector".to_string());
        assert_eq!(config_response.protocol_admin, "protocol_admin".to_string());
        assert_eq!(config_response.accepted_in_denom, "in".to_string());

        // random user cant update config
        let mut env = mock_env();
        let info = mock_info("random", &[]);
        env.block.time = Timestamp::from_seconds(0);
        let msg = crate::msg::ExecuteMsg::UpdateConfig {
            min_stream_duration: Some(Uint64::new(2000)),
            min_duration_until_start_time: Some(Uint64::new(2000)),
            stream_creation_denom: Some("fee2".to_string()),
            stream_creation_fee: Some(Uint128::new(200)),
            fee_collector: Some("collector2".to_string()),
            accepted_in_denom: Some("new_denom".to_string()),
            exit_fee_percent: Some(Decimal256::percent(2)),
        };
        let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(res, ContractError::Unauthorized {});

        // wrong fee amount
        let mut env = mock_env();
        let info = mock_info("protocol_admin", &[]);
        env.block.time = Timestamp::from_seconds(0);
        let msg = crate::msg::ExecuteMsg::UpdateConfig {
            min_stream_duration: Some(Uint64::new(2000)),
            min_duration_until_start_time: Some(Uint64::new(2000)),
            stream_creation_denom: Some("fee2".to_string()),
            stream_creation_fee: Some(Uint128::new(0)),
            fee_collector: Some("collector2".to_string()),
            accepted_in_denom: Some("new_denom".to_string()),
            exit_fee_percent: Some(Decimal256::percent(2)),
        };
        let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(res, ContractError::InvalidStreamCreationFee {});

        // wrong exit fee percent
        let mut env = mock_env();
        let info = mock_info("protocol_admin", &[]);
        env.block.time = Timestamp::from_seconds(0);
        let msg = crate::msg::ExecuteMsg::UpdateConfig {
            min_stream_duration: Some(Uint64::new(2000)),
            min_duration_until_start_time: Some(Uint64::new(2000)),
            stream_creation_denom: Some("fee2".to_string()),
            stream_creation_fee: Some(Uint128::new(200)),
            fee_collector: Some("collector2".to_string()),
            accepted_in_denom: Some("new_denom".to_string()),
            exit_fee_percent: Some(Decimal256::percent(101)),
        };
        let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(res, ContractError::InvalidExitFeePercent {});

        // protocol admin can update config
        let mut env = mock_env();
        let info = mock_info("protocol_admin", &[]);
        env.block.time = Timestamp::from_seconds(0);
        let msg = crate::msg::ExecuteMsg::UpdateConfig {
            min_stream_duration: Some(Uint64::new(2000)),
            min_duration_until_start_time: Some(Uint64::new(2000)),
            stream_creation_denom: Some("fee2".to_string()),
            stream_creation_fee: Some(Uint128::new(200)),
            fee_collector: Some("collector2".to_string()),
            accepted_in_denom: Some("new_denom".to_string()),
            exit_fee_percent: Some(Decimal256::percent(2)),
        };
        execute(deps.as_mut(), env, info, msg).unwrap();

        //query config
        let config_response = query_config(deps.as_ref()).unwrap();
        //check config
        assert_eq!(config_response.min_stream_seconds, Uint64::new(2000));
        assert_eq!(
            config_response.min_seconds_until_start_time,
            Uint64::new(2000)
        );
        assert_eq!(config_response.stream_creation_denom, "fee2".to_string());
        assert_eq!(config_response.stream_creation_fee, Uint128::new(200));
        assert_eq!(config_response.fee_collector, "collector2".to_string());
        assert_eq!(config_response.protocol_admin, "protocol_admin".to_string());
        assert_eq!(config_response.accepted_in_denom, "new_denom".to_string());
        assert_eq!(config_response.exit_fee_percent, Decimal256::percent(2));

        // create stream
        let out_supply = Uint256::from(1000u128);
        let out_denom = "out";
        let start = Timestamp::from_seconds(10000);
        let end = Timestamp::from_seconds(1000000);
        let treasury = "treasury";
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(0);
        let info = mock_info(
            "creator1",
            &[
                Coin::new(out_supply.to_string().parse().unwrap(), out_denom),
                Coin::new(200, "fee2"),
            ],
        );
        execute_create_stream(
            deps.as_mut(),
            env,
            info,
            treasury.to_string(),
            "test".to_string(),
            Some("https://sample.url".to_string()),
            "new_denom".to_string(),
            out_denom.to_string(),
            out_supply,
            start,
            end,
            None,
        )
        .unwrap();

        // update config during stream
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(100000);
        let info = mock_info("protocol_admin", &[]);
        let msg = crate::msg::ExecuteMsg::UpdateConfig {
            min_stream_duration: Some(Uint64::new(3000)),
            min_duration_until_start_time: Some(Uint64::new(4000)),
            stream_creation_denom: Some("fee3".to_string()),
            stream_creation_fee: Some(Uint128::new(300)),
            fee_collector: Some("collector3".to_string()),
            accepted_in_denom: Some("new_denom2".to_string()),
            exit_fee_percent: Some(Decimal256::percent(5)),
        };
        execute(deps.as_mut(), env, info, msg).unwrap();
        //query config
        let config_response = query_config(deps.as_ref()).unwrap();
        //check config
        assert_eq!(config_response.min_stream_seconds, Uint64::new(3000));
        assert_eq!(
            config_response.min_seconds_until_start_time,
            Uint64::new(4000)
        );
        assert_eq!(config_response.stream_creation_denom, "fee3".to_string());
        assert_eq!(config_response.stream_creation_fee, Uint128::new(300));
        assert_eq!(config_response.fee_collector, "collector3".to_string());
        assert_eq!(config_response.protocol_admin, "protocol_admin".to_string());
        assert_eq!(config_response.accepted_in_denom, "new_denom2".to_string());
        assert_eq!(config_response.exit_fee_percent, Decimal256::percent(5));

        // check stream
        let mut env = mock_env();
        env.block.time = Timestamp::from_seconds(100000);
        let stream_response = query_stream(deps.as_ref(), env, 1).unwrap();
        assert_eq!(stream_response.exit_fee_percent, Decimal256::percent(2));
        assert_eq!(stream_response.stream_creation_fee, Uint128::new(200));
    }

    #[cfg(test)]
    mod killswitch {
        use super::*;
        use crate::contract::{list_positions, list_streams};
        use crate::killswitch::{
            execute_cancel_stream, execute_exit_cancelled, execute_resume_stream,
            sudo_cancel_stream, sudo_pause_stream,
        };
        use cosmwasm_std::CosmosMsg::Bank;
        use cosmwasm_std::{ReplyOn, SubMsg};

        #[test]
        fn test_pause_protocol_admin() {
            let treasury = Addr::unchecked("treasury");
            let start = Timestamp::from_seconds(1_000_000);
            let end = Timestamp::from_seconds(5_000_000);
            let out_supply = Uint256::from(1_000_000_000_000u128);
            let out_denom = "out_denom";

            // instantiate
            let mut deps = mock_dependencies();
            let mut env = mock_env();
            env.block.time = Timestamp::from_seconds(0);
            let msg = crate::msg::InstantiateMsg {
                min_stream_seconds: Uint64::new(1000),
                min_seconds_until_start_time: Uint64::new(0),
                stream_creation_denom: "fee".to_string(),
                stream_creation_fee: Uint128::new(100),
                exit_fee_percent: Decimal256::percent(1),
                fee_collector: "collector".to_string(),
                protocol_admin: "protocol_admin".to_string(),
                accepted_in_denom: "in".to_string(),
            };
            instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

            // create stream
            let mut env = mock_env();
            env.block.time = Timestamp::from_seconds(0);
            let info = mock_info(
                "creator1",
                &[
                    Coin::new(out_supply.to_string().parse().unwrap(), out_denom),
                    Coin::new(100, "fee"),
                ],
            );
            execute_create_stream(
                deps.as_mut(),
                env,
                info,
                treasury.to_string(),
                "test".to_string(),
                Some("https://sample.url".to_string()),
                "in".to_string(),
                out_denom.to_string(),
                out_supply,
                start,
                end,
                None,
            )
            .unwrap();

            // non protocol admin can't pause
            let info = mock_info("non_protocol_admin", &[]);
            let mut env = mock_env();
            env.block.time = start.plus_seconds(100);

            let res = execute_pause_stream(deps.as_mut(), env, info, 1);
            assert_eq!(res, Err(ContractError::Unauthorized {}));

            // first subscription
            let mut env = mock_env();
            env.block.time = start.plus_seconds(1_000_000);
            let funds = Coin::new(3_000, "in");
            let info = mock_info("position1", &[funds]);
            let msg = crate::msg::ExecuteMsg::Subscribe {
                stream_id: 1,
                operator_target: None,
                operator: None,
            };
            let _res = execute(deps.as_mut(), env, info, msg).unwrap();

            //can't pause before start time
            let info = mock_info("protocol_admin", &[]);
            let mut env = mock_env();
            env.block.time = start.minus_seconds(500_000);
            let res = execute_pause_stream(deps.as_mut(), env, info, 1).unwrap_err();
            assert_eq!(res, ContractError::StreamNotStarted {});

            // can't pause after end time
            let info = mock_info("protocol_admin", &[]);
            let mut env = mock_env();
            env.block.time = end.plus_seconds(500_000);
            let res = execute_pause_stream(deps.as_mut(), env, info, 1).unwrap_err();
            assert_eq!(res, ContractError::StreamEnded {});

            // protocol admin can pause
            let info = mock_info("protocol_admin", &[]);
            let mut env = mock_env();
            env.block.time = start.plus_seconds(1_000_001);
            execute_pause_stream(deps.as_mut(), env, info, 1).unwrap();

            // can't paused if already paused
            let info = mock_info("protocol_admin", &[]);
            let mut env = mock_env();
            env.block.time = start.plus_seconds(1_000_005);
            let res = execute_pause_stream(deps.as_mut(), env, info, 1).unwrap_err();
            assert_eq!(res, ContractError::StreamKillswitchActive {});

            // can't subscribe new
            let mut env = mock_env();
            env.block.time = start.plus_seconds(1_000_002);
            let funds = Coin::new(3_000, "in");
            let info = mock_info("position2", &[funds]);
            let msg = crate::msg::ExecuteMsg::Subscribe {
                stream_id: 1,
                operator_target: None,
                operator: None,
            };
            let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
            assert_eq!(res, ContractError::StreamKillswitchActive {});

            // can't subscribe more
            let mut env = mock_env();
            env.block.time = start.plus_seconds(1_000_002);
            let funds = Coin::new(3_000, "in");
            let info = mock_info("position1", &[funds]);
            let msg = crate::msg::ExecuteMsg::Subscribe {
                stream_id: 1,
                operator_target: None,
                operator: None,
            };
            let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
            assert_eq!(res, ContractError::StreamKillswitchActive {});

            // can't withdraw
            let mut env = mock_env();
            env.block.time = start.plus_seconds(1_000_002);
            let info = mock_info("position1", &[]);
            let msg = crate::msg::ExecuteMsg::Withdraw {
                stream_id: 1,
                cap: None,
                operator_target: None,
            };
            let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
            assert_eq!(res, ContractError::StreamKillswitchActive {});

            // can't update stream
            let mut env = mock_env();
            env.block.time = start.plus_seconds(1_000_002);
            let res = execute_update_stream(deps.as_mut(), env, 1);
            assert_eq!(res, Err(ContractError::StreamPaused {}));

            // can't update position
            let mut env = mock_env();
            env.block.time = start.plus_seconds(1_000_002);
            let info = mock_info("position1", &[]);
            let res = execute_update_position(deps.as_mut(), env, info, 1, None);
            assert_eq!(res, Err(ContractError::StreamPaused {}));

            // can't finalize stream
            let mut env = mock_env();
            env.block.time = end.plus_seconds(1_000_002);
            let info = mock_info("treasury", &[]);
            let res = execute_finalize_stream(deps.as_mut(), env, info, 1, None);
            assert_eq!(res, Err(ContractError::StreamKillswitchActive {}));

            // can't exit
            let mut env = mock_env();
            env.block.time = end.plus_seconds(1_000_002);
            let info = mock_info("position1", &[]);
            let res = execute_exit_stream(deps.as_mut(), env, info, 1, None);
            assert_eq!(res, Err(ContractError::StreamKillswitchActive {}));
        }

        #[test]
        fn test_resume_protocol_admin() {
            let treasury = Addr::unchecked("treasury");
            let start = Timestamp::from_seconds(1_000_000);
            let end = Timestamp::from_seconds(5_000_000);
            let out_supply = Uint256::from(1_000_000_000_000u128);
            let out_denom = "out_denom";

            // instantiate
            let mut deps = mock_dependencies();
            let mut env = mock_env();
            env.block.time = Timestamp::from_seconds(0);
            let msg = crate::msg::InstantiateMsg {
                min_stream_seconds: Uint64::new(1000),
                min_seconds_until_start_time: Uint64::new(0),
                stream_creation_denom: "fee".to_string(),
                stream_creation_fee: Uint128::new(100),
                exit_fee_percent: Decimal256::percent(1),
                fee_collector: "collector".to_string(),
                protocol_admin: "protocol_admin".to_string(),
                accepted_in_denom: "in".to_string(),
            };
            instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

            // create stream
            let mut env = mock_env();
            env.block.time = Timestamp::from_seconds(0);
            let info = mock_info(
                "creator1",
                &[
                    Coin::new(out_supply.to_string().parse().unwrap(), out_denom),
                    Coin::new(100, "fee"),
                ],
            );
            execute_create_stream(
                deps.as_mut(),
                env,
                info,
                treasury.to_string(),
                "test".to_string(),
                Some("https://sample.url".to_string()),
                "in".to_string(),
                out_denom.to_string(),
                out_supply,
                start,
                end,
                None,
            )
            .unwrap();

            // first subscription
            let mut env = mock_env();
            env.block.time = start.plus_seconds(1_000_000);
            let funds = Coin::new(3_000, "in");
            let info = mock_info("position1", &[funds]);
            let msg = crate::msg::ExecuteMsg::Subscribe {
                stream_id: 1,
                operator_target: None,
                operator: None,
            };
            let _res = execute(deps.as_mut(), env, info, msg).unwrap();

            // can't resume if not paused
            let info = mock_info("protocol_admin", &[]);
            let mut env = mock_env();
            env.block.time = start.plus_seconds(1_000_003);
            let res = execute_resume_stream(deps.as_mut(), env, info, 1).unwrap_err();
            assert_eq!(res, ContractError::StreamNotPaused {});

            // protocol admin can pause
            let info = mock_info("protocol_admin", &[]);
            let mut env = mock_env();
            env.block.time = start.plus_seconds(1_000_001);
            execute_pause_stream(deps.as_mut(), env, info, 1).unwrap();

            // can't subscribe new
            let mut env = mock_env();
            env.block.time = start.plus_seconds(1_000_002);
            let funds = Coin::new(3_000, "in");
            let info = mock_info("position2", &[funds]);
            let msg = crate::msg::ExecuteMsg::Subscribe {
                stream_id: 1,
                operator_target: None,
                operator: None,
            };
            let res = execute(deps.as_mut(), env, info, msg).unwrap_err();
            assert_eq!(res, ContractError::StreamKillswitchActive {});

            // non protocol admin can't resume
            let info = mock_info("non_protocol_admin", &[]);
            let mut env = mock_env();
            env.block.time = start.plus_seconds(1_000_003);
            let res = execute_resume_stream(deps.as_mut(), env, info, 1).unwrap_err();
            assert_eq!(res, ContractError::Unauthorized {});

            // protocol admin can resume
            let info = mock_info("protocol_admin", &[]);
            let mut env = mock_env();
            env.block.time = start.plus_seconds(1_000_003);
            execute_resume_stream(deps.as_mut(), env, info, 1).unwrap();

            // can subscribe new after resume
            let mut env = mock_env();
            env.block.time = start.plus_seconds(1_000_004);
            let funds = Coin::new(3_000, "in");
            let info = mock_info("position2", &[funds]);
            let msg = crate::msg::ExecuteMsg::Subscribe {
                stream_id: 1,
                operator_target: None,
                operator: None,
            };
            let res = execute(deps.as_mut(), env, info, msg).unwrap();
            assert_eq!(res.attributes[0].key, "action");
            assert_eq!(res.attributes[0].value, "subscribe");
            assert_eq!(res.attributes[1].key, "stream_id");
            assert_eq!(res.attributes[1].value, "1");
            assert_eq!(res.attributes[2].key, "owner");
            assert_eq!(res.attributes[2].value, "position2");
            assert_eq!(res.attributes[3].key, "in_supply");
            assert_eq!(res.attributes[3].value, "6000");
            assert_eq!(res.attributes[4].key, "in_amount");
            assert_eq!(res.attributes[4].value, "3000");

            // protocol admin can pause
            let info = mock_info("protocol_admin", &[]);
            let mut env = mock_env();
            env.block.time = start.plus_seconds(1_000_005);
            execute_pause_stream(deps.as_mut(), env, info, 1).unwrap();

            // cancel the stream
            let info = mock_info("protocol_admin", &[]);
            let mut env = mock_env();
            env.block.time = start.plus_seconds(1_000_006);
            execute_cancel_stream(deps.as_mut(), env, info, 1).unwrap();

            // can't resume if cancelled
            let info = mock_info("protocol_admin", &[]);
            let mut env = mock_env();
            env.block.time = start.plus_seconds(1_000_007);
            let res = execute_resume_stream(deps.as_mut(), env, info, 1).unwrap_err();
            assert_eq!(res, ContractError::StreamIsCancelled {});
        }

        #[test]
        fn test_cancel_protocol_admin() {
            let treasury = Addr::unchecked("treasury");
            let start = Timestamp::from_seconds(1_000_000);
            let end = Timestamp::from_seconds(5_000_000);
            let out_supply = Uint256::from(1_000_000_000_000u128);
            let out_denom = "out_denom";

            // instantiate
            let mut deps = mock_dependencies();
            let mut env = mock_env();
            env.block.time = Timestamp::from_seconds(0);
            let msg = crate::msg::InstantiateMsg {
                min_stream_seconds: Uint64::new(1000),
                min_seconds_until_start_time: Uint64::new(0),
                stream_creation_denom: "fee".to_string(),
                stream_creation_fee: Uint128::new(100),
                exit_fee_percent: Decimal256::percent(1),
                fee_collector: "collector".to_string(),
                protocol_admin: "protocol_admin".to_string(),
                accepted_in_denom: "in".to_string(),
            };
            instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

            // create stream
            let mut env = mock_env();
            env.block.time = Timestamp::from_seconds(0);
            let info = mock_info(
                "creator1",
                &[
                    Coin::new(out_supply.to_string().parse().unwrap(), out_denom),
                    Coin::new(100, "fee"),
                ],
            );
            execute_create_stream(
                deps.as_mut(),
                env,
                info,
                treasury.to_string(),
                "test".to_string(),
                Some("https://sample.url".to_string()),
                "in".to_string(),
                out_denom.to_string(),
                out_supply,
                start,
                end,
                None,
            )
            .unwrap();

            // subscription
            let mut env = mock_env();
            env.block.time = start.plus_seconds(0);
            let funds = Coin::new(2_000_000_000_000, "in");
            let info = mock_info("creator1", &[funds]);
            let msg = crate::msg::ExecuteMsg::Subscribe {
                stream_id: 1,
                operator_target: None,
                operator: Some("operator".to_string()),
            };
            let _res = execute(deps.as_mut(), env, info, msg).unwrap();

            // non protocol admin can't cancel
            let info = mock_info("non_protocol_admin", &[]);
            let mut env = mock_env();
            env.block.time = start.plus_seconds(1_000_000);
            let err = execute_cancel_stream(deps.as_mut(), env, info, 1).unwrap_err();
            assert_eq!(err, ContractError::Unauthorized {});

            // cant cancel without pause
            let info = mock_info("protocol_admin", &[]);
            let mut env = mock_env();
            env.block.time = start.plus_seconds(1_000_000);
            let err = execute_cancel_stream(deps.as_mut(), env, info, 1).unwrap_err();
            assert_eq!(err, ContractError::StreamNotPaused {});

            // pause
            let mut env = mock_env();
            env.block.time = start.plus_seconds(2_000_000);
            let info = mock_info("protocol_admin", &[]);
            execute_pause_stream(deps.as_mut(), env, info, 1).unwrap();

            //cancel
            let info = mock_info("protocol_admin", &[]);
            let mut env = mock_env();
            env.block.time = start.plus_seconds(2_500_000);
            let response = execute_cancel_stream(deps.as_mut(), env, info, 1).unwrap();
            //out_tokens and the creation fee are sent back to the treasury upon cancellation
            assert_eq!(
                response.messages,
                [
                    SubMsg {
                        id: 0,
                        msg: Bank(BankMsg::Send {
                            to_address: "treasury".to_string(),
                            amount: Vec::from([Coin {
                                denom: "out_denom".to_string(),
                                amount: Uint128::new(1000000000000)
                            }])
                        }),
                        gas_limit: None,
                        reply_on: ReplyOn::Never
                    },
                    SubMsg {
                        id: 0,
                        msg: Bank(BankMsg::Send {
                            to_address: "treasury".to_string(),
                            amount: Vec::from([Coin {
                                denom: "fee".to_string(),
                                amount: Uint128::new(100)
                            }])
                        }),
                        gas_limit: None,
                        reply_on: ReplyOn::Never
                    }
                ]
            );

            // can't cancel cancelled stream
            let info = mock_info("protocol_admin", &[]);
            let mut env = mock_env();
            env.block.time = start.plus_seconds(2_500_000);
            let response = execute_cancel_stream(deps.as_mut(), env, info, 1).unwrap_err();
            assert_eq!(response, ContractError::StreamIsCancelled {});
        }

        #[test]
        fn test_withdraw_pause() {
            let treasury = Addr::unchecked("treasury");
            let start = Timestamp::from_seconds(1_000_000);
            let end = Timestamp::from_seconds(5_000_000);
            let out_supply = Uint256::from(1_000_000_000_000u128);
            let out_denom = "out_denom";

            // instantiate
            let mut deps = mock_dependencies();
            let mut env = mock_env();
            env.block.time = Timestamp::from_seconds(0);
            let msg = crate::msg::InstantiateMsg {
                min_stream_seconds: Uint64::new(1000),
                min_seconds_until_start_time: Uint64::new(0),
                stream_creation_denom: "fee".to_string(),
                stream_creation_fee: Uint128::new(100),
                exit_fee_percent: Decimal256::percent(1),
                fee_collector: "collector".to_string(),
                protocol_admin: "protocol_admin".to_string(),
                accepted_in_denom: "in".to_string(),
            };
            instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

            // create stream
            let mut env = mock_env();
            env.block.time = Timestamp::from_seconds(0);
            let info = mock_info(
                "creator1",
                &[
                    Coin::new(out_supply.to_string().parse().unwrap(), out_denom),
                    Coin::new(100, "fee"),
                ],
            );
            execute_create_stream(
                deps.as_mut(),
                env,
                info,
                treasury.to_string(),
                "test".to_string(),
                Some("https://sample.url".to_string()),
                "in".to_string(),
                out_denom.to_string(),
                out_supply,
                start,
                end,
                None,
            )
            .unwrap();

            // subscription
            let mut env = mock_env();
            env.block.time = start.plus_seconds(0);
            let funds = Coin::new(2_000_000_000_000, "in");
            let info = mock_info("creator1", &[funds.clone()]);
            let msg = crate::msg::ExecuteMsg::Subscribe {
                stream_id: 1,
                operator_target: None,
                operator: Some("operator".to_string()),
            };
            let _res = execute(deps.as_mut(), env, info, msg).unwrap();

            // withdraw with cap
            let mut env = mock_env();
            env.block.time = start.plus_seconds(5000);
            let info = mock_info("creator1", &[]);
            let cap = Uint256::from(25_000_000u128);
            let msg = crate::msg::ExecuteMsg::Withdraw {
                stream_id: 1,
                cap: Some(cap),
                operator_target: None,
            };
            let _res = execute(deps.as_mut(), env, info, msg).unwrap();

            let position =
                query_position(deps.as_ref(), mock_env(), 1, "creator1".to_string()).unwrap();
            assert_eq!(position.in_balance, Uint256::from(1_997_475_000_000u128));
            assert_eq!(position.spent, Uint256::from(2_500_000_000u128));
            assert_eq!(position.purchased, Uint256::from(1_250_000_000u128));
            // first fund amount should be equal to in_balance + spent + cap
            assert_eq!(
                position.in_balance + position.spent + cap,
                Uint256::from_str(funds.amount.to_string().as_str()).unwrap()
            );

            // can't withdraw pause
            let mut env = mock_env();
            env.block.time = start.plus_seconds(6000);
            let info = mock_info("creator1", &[]);
            let err = execute_withdraw_paused(deps.as_mut(), env, info, 1, None, None).unwrap_err();
            assert_eq!(err, ContractError::StreamNotPaused {});

            // pause
            let mut env = mock_env();
            env.block.time = start.plus_seconds(6000);
            let info = mock_info("protocol_admin", &[]);
            execute_pause_stream(deps.as_mut(), env, info, 1).unwrap();

            let mut env = mock_env();
            env.block.time = start.plus_seconds(6500);
            let stream1_old = query_stream(deps.as_ref(), env, 1).unwrap();
            //Unauthorized check
            let info = mock_info("random", &[]);
            let mut env = mock_env();
            env.block.time = start.plus_seconds(7000);
            let res = execute_withdraw_paused(
                deps.as_mut(),
                env,
                info,
                1,
                None,
                Some("creator1".to_string()),
            )
            .unwrap_err();

            assert_eq!(res, ContractError::Unauthorized {});
            //Cap exceeds in balance check
            let mut env = mock_env();
            env.block.time = start.plus_seconds(7000);
            let info = mock_info("creator1", &[]);
            let res = execute_withdraw_paused(
                deps.as_mut(),
                env,
                info,
                1,
                Some(Uint256::from(2_000_000_000_000u128 + 1u128)),
                None,
            )
            .unwrap_err();
            assert_eq!(
                res,
                ContractError::WithdrawAmountExceedsBalance(Uint256::from(2_000_000_000_001u128))
            );
            // Withdraw cap is zero
            let mut env = mock_env();
            env.block.time = start.plus_seconds(7000);
            let info = mock_info("creator1", &[]);
            let res =
                execute_withdraw_paused(deps.as_mut(), env, info, 1, Some(Uint256::zero()), None)
                    .unwrap_err();
            assert_eq!(res, ContractError::InvalidWithdrawAmount {});

            //withdraw with cap
            let mut env = mock_env();
            env.block.time = start.plus_seconds(7000);
            let info = mock_info("creator1", &[]);
            let cap = Uint256::from(25_000_000u128);
            execute_withdraw_paused(deps.as_mut(), env, info, 1, Some(cap), None).unwrap();

            // withdraw after pause
            let mut env = mock_env();
            env.block.time = start.plus_seconds(7000);
            let info = mock_info("creator1", &[]);
            let res = execute_withdraw_paused(deps.as_mut(), env, info, 1, None, None).unwrap();
            assert_eq!(
                res.messages,
                vec![SubMsg {
                    id: 0,
                    msg: BankMsg::Send {
                        to_address: "creator1".to_string(),
                        amount: vec![Coin {
                            denom: "in".to_string(),
                            amount: Uint128::new(1996950006258),
                        }],
                    }
                    .into(),
                    gas_limit: None,
                    reply_on: ReplyOn::Never,
                }]
            );

            // stream not updated
            let mut env = mock_env();
            env.block.time = start.plus_seconds(8000);
            let stream1_new = query_stream(deps.as_ref(), env, 1).unwrap();
            // dist_index not updated
            assert_eq!(stream1_old.dist_index, stream1_new.dist_index);
            assert_eq!(stream1_new.in_supply, Uint256::zero());
            assert_eq!(stream1_new.shares, Uint256::zero());

            // position updated
            let mut env = mock_env();
            env.block.time = start.plus_seconds(8001);
            let position =
                query_position(deps.as_ref(), mock_env(), 1, "creator1".to_string()).unwrap();
            // in_balance updated
            assert_eq!(position.in_balance, Uint256::zero());
            assert_eq!(position.spent, Uint256::from(2_999_993_742u128));
            assert_eq!(position.purchased, Uint256::from(1_499_999_998u128));
            assert_eq!(position.shares, Uint256::zero());
        }

        #[test]
        fn test_resume() {
            let treasury = Addr::unchecked("treasury");
            let start = Timestamp::from_seconds(1_000_000);
            let end = Timestamp::from_seconds(5_000_000);
            let out_supply = Uint256::from(1_000_000_000_000u128);
            let out_denom = "out_denom";

            // instantiate
            let mut deps = mock_dependencies();
            let mut env = mock_env();
            env.block.time = Timestamp::from_seconds(0);
            let msg = crate::msg::InstantiateMsg {
                min_stream_seconds: Uint64::new(1000),
                min_seconds_until_start_time: Uint64::new(0),
                stream_creation_denom: "fee".to_string(),
                stream_creation_fee: Uint128::new(100),
                exit_fee_percent: Decimal256::percent(1),
                fee_collector: "collector".to_string(),
                protocol_admin: "protocol_admin".to_string(),
                accepted_in_denom: "in".to_string(),
            };
            instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

            // create stream
            let mut env = mock_env();
            env.block.time = Timestamp::from_seconds(0);
            let info = mock_info(
                "creator1",
                &[
                    Coin::new(out_supply.to_string().parse().unwrap(), out_denom),
                    Coin::new(100, "fee"),
                ],
            );
            execute_create_stream(
                deps.as_mut(),
                env,
                info,
                treasury.to_string(),
                "test".to_string(),
                Some("https://sample.url".to_string()),
                "in".to_string(),
                out_denom.to_string(),
                out_supply,
                start,
                end,
                None,
            )
            .unwrap();

            // first subscription
            let mut env = mock_env();
            env.block.time = start.plus_seconds(1_000_000);
            let funds = Coin::new(3_000, "in");
            let info = mock_info("position1", &[funds]);
            let msg = crate::msg::ExecuteMsg::Subscribe {
                stream_id: 1,
                operator_target: None,
                operator: None,
            };
            let _res = execute(deps.as_mut(), env, info, msg).unwrap();

            //cant resume if not paused
            let mut env = mock_env();
            env.block.time = start.plus_seconds(1_000_000);
            let res = sudo_resume_stream(deps.as_mut(), env, 1).unwrap_err();
            assert_eq!(res, ContractError::StreamNotPaused {});

            // pause
            let info = mock_info("protocol_admin", &[]);
            let mut env = mock_env();
            let pause_date = start.plus_seconds(2_000_000);
            env.block.time = pause_date;
            execute_pause_stream(deps.as_mut(), env, info, 1).unwrap();

            // resume
            let mut env = mock_env();
            let resume_date = start.plus_seconds(3_000_000);
            env.block.time = resume_date;
            sudo_resume_stream(deps.as_mut(), env, 1).unwrap();

            // new end date is correct
            let new_end_date = end.plus_nanos(resume_date.nanos() - pause_date.nanos());
            let stream = query_stream(deps.as_ref(), mock_env(), 1).unwrap();
            assert_eq!(stream.end_time, new_end_date);
        }

        #[test]
        fn test_sudo_pause_stream() {
            let treasury = Addr::unchecked("treasury");
            let start = Timestamp::from_seconds(1_000_000);
            let end = Timestamp::from_seconds(5_000_000);
            let out_supply = Uint256::from(1_000_000_000_000u128);
            let out_denom = "out_denom";

            // instantiate
            let mut deps = mock_dependencies();
            let mut env = mock_env();
            env.block.time = Timestamp::from_seconds(0);
            let msg = crate::msg::InstantiateMsg {
                min_stream_seconds: Uint64::new(1000),
                min_seconds_until_start_time: Uint64::new(0),
                stream_creation_denom: "fee".to_string(),
                stream_creation_fee: Uint128::new(100),
                exit_fee_percent: Decimal256::percent(1),
                fee_collector: "collector".to_string(),
                protocol_admin: "protocol_admin".to_string(),
                accepted_in_denom: "in".to_string(),
            };
            instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

            // create stream
            let mut env = mock_env();
            env.block.time = Timestamp::from_seconds(0);
            let info = mock_info(
                "creator1",
                &[
                    Coin::new(out_supply.to_string().parse().unwrap(), out_denom),
                    Coin::new(100, "fee"),
                ],
            );
            execute_create_stream(
                deps.as_mut(),
                env,
                info,
                treasury.to_string(),
                "test".to_string(),
                Some("https://sample.url".to_string()),
                "in".to_string(),
                out_denom.to_string(),
                out_supply,
                start,
                end,
                None,
            )
            .unwrap();

            let mut env = mock_env();
            env.block.time = Timestamp::from_seconds(500_000);
            let res = sudo_pause_stream(deps.as_mut(), env, 1).unwrap_err();
            assert_eq!(res, ContractError::StreamNotStarted {});

            let mut env = mock_env();
            env.block.time = Timestamp::from_seconds(6_000_000);
            let res = sudo_pause_stream(deps.as_mut(), env, 1).unwrap_err();
            assert_eq!(res, ContractError::StreamEnded {});

            let mut env = mock_env();
            env.block.time = Timestamp::from_seconds(3_000_000);
            let res = sudo_pause_stream(deps.as_mut(), env, 1).unwrap();
            assert_eq!(
                res,
                Response::new()
                    .add_attribute("action", "sudo_pause_stream")
                    .add_attribute("stream_id", "1")
                    .add_attribute("is_paused", "true")
                    .add_attribute("pause_date", "3000000.000000000")
            );

            let mut env = mock_env();
            env.block.time = Timestamp::from_seconds(4_000_000);
            let res = sudo_pause_stream(deps.as_mut(), env, 1).unwrap_err();
            assert_eq!(res, ContractError::StreamKillswitchActive {});
        }

        #[test]
        fn test_range_queries() {
            let treasury = Addr::unchecked("treasury");
            let start = Timestamp::from_seconds(2000);
            let end = Timestamp::from_seconds(1_000_000);
            let out_supply = Uint256::from(1_000_000u128);
            let out_denom = "out_denom";

            // instantiate
            let mut deps = mock_dependencies();
            let mut env = mock_env();
            env.block.time = Timestamp::from_seconds(100);
            let msg = crate::msg::InstantiateMsg {
                min_stream_seconds: Uint64::new(1000),
                min_seconds_until_start_time: Uint64::new(1000),
                stream_creation_denom: "fee".to_string(),
                stream_creation_fee: Uint128::new(100),
                exit_fee_percent: Decimal256::percent(1),
                fee_collector: "collector".to_string(),
                protocol_admin: "protocol_admin".to_string(),
                accepted_in_denom: "in".to_string(),
            };
            instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

            // create stream
            let mut env = mock_env();
            env.block.time = Timestamp::from_seconds(1);
            let info = mock_info(
                "creator1",
                &[
                    Coin::new(out_supply.to_string().parse().unwrap(), out_denom),
                    Coin::new(100, "fee"),
                ],
            );
            //first stream
            execute_create_stream(
                deps.as_mut(),
                env.clone(),
                info.clone(),
                treasury.to_string(),
                "test".to_string(),
                Some("https://sample.url".to_string()),
                "in".to_string(),
                out_denom.to_string(),
                out_supply,
                start,
                end,
                None,
            )
            .unwrap();
            //second stream
            execute_create_stream(
                deps.as_mut(),
                env,
                info,
                treasury.to_string(),
                "test".to_string(),
                Some("https://sample.url".to_string()),
                "in".to_string(),
                out_denom.to_string(),
                out_supply,
                start,
                end,
                None,
            )
            .unwrap();

            let res = list_streams(deps.as_ref(), None, None).unwrap();
            assert_eq!(res.streams.len(), 2);

            // first subscription to first stream
            let mut env = mock_env();
            env.block.time = start.plus_seconds(100);
            let info = mock_info("creator1", &[Coin::new(1_000_000, "in")]);
            let msg = crate::msg::ExecuteMsg::Subscribe {
                stream_id: 1,
                operator_target: None,
                operator: None,
            };
            let _res = execute(deps.as_mut(), env, info, msg).unwrap();

            // second subscription to first stream
            let mut env = mock_env();
            env.block.time = start.plus_seconds(100);
            let info = mock_info("creator2", &[Coin::new(1_000_000, "in")]);
            let msg = crate::msg::ExecuteMsg::Subscribe {
                stream_id: 1,
                operator_target: None,
                operator: None,
            };
            let _res = execute(deps.as_mut(), env, info, msg).unwrap();

            let res = list_positions(deps.as_ref(), 1, None, None).unwrap();
            assert_eq!(res.positions.len(), 2);
        }

        #[test]
        fn test_exit_cancel() {
            let treasury = Addr::unchecked("treasury");
            let start = Timestamp::from_seconds(1_000_000);
            let end = Timestamp::from_seconds(5_000_000);
            let out_supply = Uint256::from(1_000_000_000_000u128);
            let out_denom = "out_denom";

            // instantiate
            let mut deps = mock_dependencies();
            let mut env = mock_env();
            env.block.time = Timestamp::from_seconds(0);
            let msg = crate::msg::InstantiateMsg {
                min_stream_seconds: Uint64::new(1000),
                min_seconds_until_start_time: Uint64::new(0),
                stream_creation_denom: "fee".to_string(),
                stream_creation_fee: Uint128::new(100),
                exit_fee_percent: Decimal256::percent(1),
                fee_collector: "collector".to_string(),
                protocol_admin: "protocol_admin".to_string(),
                accepted_in_denom: "in".to_string(),
            };
            instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

            // create stream
            let mut env = mock_env();
            env.block.time = Timestamp::from_seconds(0);
            let info = mock_info(
                "creator1",
                &[
                    Coin::new(out_supply.to_string().parse().unwrap(), out_denom),
                    Coin::new(100, "fee"),
                ],
            );
            execute_create_stream(
                deps.as_mut(),
                env,
                info,
                treasury.to_string(),
                "test".to_string(),
                Some("https://sample.url".to_string()),
                "in".to_string(),
                out_denom.to_string(),
                out_supply,
                start,
                end,
                None,
            )
            .unwrap();

            // subscription
            let mut env = mock_env();
            env.block.time = start.plus_seconds(0);
            let funds = Coin::new(2_000_000_000_000, "in");
            let info = mock_info("creator1", &[funds]);
            let msg = crate::msg::ExecuteMsg::Subscribe {
                stream_id: 1,
                operator_target: None,
                operator: Some("operator".to_string()),
            };
            let _res = execute(deps.as_mut(), env, info, msg).unwrap();

            // cant cancel without pause
            let mut env = mock_env();
            env.block.time = start.plus_seconds(1_000_000);
            let err = sudo_cancel_stream(deps.as_mut(), env, 1).unwrap_err();
            assert_eq!(err, ContractError::StreamNotPaused {});

            // pause
            let mut env = mock_env();
            env.block.time = start.plus_seconds(2_000_000);
            let info = mock_info("protocol_admin", &[]);
            execute_pause_stream(deps.as_mut(), env, info, 1).unwrap();

            //can't exit before cancel
            let mut env = mock_env();
            env.block.time = start.plus_seconds(2_250_000);
            let info = mock_info("creator1", &[]);
            let res = execute_exit_cancelled(deps.as_mut(), env, info, 1, None).unwrap_err();
            assert_eq!(res, ContractError::StreamNotCancelled {});

            //cancel
            let mut env = mock_env();
            env.block.time = start.plus_seconds(2_500_000);
            let response = sudo_cancel_stream(deps.as_mut(), env, 1).unwrap();
            //out_tokens and the creation fee are sent back to the treasury upon cancellation
            assert_eq!(
                response.messages,
                [
                    SubMsg {
                        id: 0,
                        msg: Bank(BankMsg::Send {
                            to_address: "treasury".to_string(),
                            amount: Vec::from([Coin {
                                denom: "out_denom".to_string(),
                                amount: Uint128::new(1000000000000)
                            }])
                        }),
                        gas_limit: None,
                        reply_on: ReplyOn::Never
                    },
                    SubMsg {
                        id: 0,
                        msg: Bank(BankMsg::Send {
                            to_address: "treasury".to_string(),
                            amount: Vec::from([Coin {
                                denom: "fee".to_string(),
                                amount: Uint128::new(100)
                            }])
                        }),
                        gas_limit: None,
                        reply_on: ReplyOn::Never
                    }
                ]
            );

            //random operator can't exit
            let mut env = mock_env();
            env.block.time = start.plus_seconds(2_250_000);
            let info = mock_info("random", &[]);
            let res =
                execute_exit_cancelled(deps.as_mut(), env, info, 1, Some("creator1".to_string()))
                    .unwrap_err();
            assert_eq!(res, ContractError::Unauthorized {});

            // exit
            let mut env = mock_env();
            env.block.time = start.plus_seconds(3_000_000);
            let info = mock_info("creator1", &[]);
            let res = execute_exit_cancelled(deps.as_mut(), env, info, 1, None).unwrap();
            let msg = res.messages.get(0).unwrap();
            assert_eq!(
                msg.msg,
                Bank(BankMsg::Send {
                    to_address: "creator1".to_string(),
                    amount: vec![Coin::new(2000000000000, "in")]
                })
            );
        }
    }
    mod threshold {
        use crate::{
            killswitch::{execute_cancel_stream_with_threshold, execute_exit_cancelled},
            threshold::ThresholdError,
        };

        // Create a stream with a threshold
        // Subscribe to the stream
        use super::*;

        #[test]
        fn test_threshold_reached() {
            let treasury = Addr::unchecked("treasury");
            let start = Timestamp::from_seconds(1_000_000);
            let end = Timestamp::from_seconds(5_000_000);
            let out_supply = Uint256::from(500u128);
            let out_denom = "out_denom";
            let in_denom = "in_denom";

            // threshold = 500*0.5 / 1-0.01 =252.5

            // instantiate
            let mut deps = mock_dependencies();
            let mut env = mock_env();
            env.block.time = Timestamp::from_seconds(0);
            let msg = crate::msg::InstantiateMsg {
                min_stream_seconds: Uint64::new(1000),
                min_seconds_until_start_time: Uint64::new(0),
                stream_creation_denom: "fee".to_string(),
                stream_creation_fee: Uint128::new(100),
                exit_fee_percent: Decimal256::percent(1),
                fee_collector: "collector".to_string(),
                protocol_admin: "protocol_admin".to_string(),
                accepted_in_denom: in_denom.to_string(),
            };
            instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

            // create stream
            let mut env = mock_env();
            env.block.time = Timestamp::from_seconds(0);
            let info = mock_info(
                "creator",
                &[
                    Coin::new(out_supply.to_string().parse().unwrap(), out_denom),
                    Coin::new(100, "fee"),
                ],
            );
            execute_create_stream(
                deps.as_mut(),
                env,
                info,
                treasury.to_string(),
                "test".to_string(),
                Some("https://sample.url".to_string()),
                in_denom.to_string(),
                out_denom.to_string(),
                out_supply,
                start,
                end,
                Some(Uint256::from(250u128)),
            )
            .unwrap();

            // subscription
            let mut env = mock_env();
            env.block.time = start;
            let funds = Coin::new(252, "in_denom");
            let info = mock_info("subscriber", &[funds]);
            let msg = crate::msg::ExecuteMsg::Subscribe {
                stream_id: 1,
                operator_target: None,
                operator: Some("operator".to_string()),
            };
            let _res = execute(deps.as_mut(), env, info, msg).unwrap();

            // Threshold should be reached
            let mut env = mock_env();
            env.block.time = end.plus_seconds(1);

            // Exit should be possible
            // Since there is only one subscriber all out denom should be sent to subscriber
            // In calculations we are always rounding down that one token will be left in the stream
            // Asuming token is 6 decimals
            // This amount could be considered as insignificant
            let info = mock_info("subscriber", &[]);
            let res = execute_exit_stream(deps.as_mut(), env.clone(), info, 1, None).unwrap();
            assert_eq!(
                res.messages,
                vec![SubMsg::new(BankMsg::Send {
                    to_address: "subscriber".to_string(),
                    amount: vec![Coin::new(499, "out_denom")],
                })],
            );

            // Creator finalizes the stream
            let info = mock_info("treasury", &[]);
            let res = execute_finalize_stream(deps.as_mut(), env.clone(), info, 1, None).unwrap();
            // Creator's revenue
            assert_eq!(
                res.messages[0].msg,
                cosmwasm_std::CosmosMsg::Bank(BankMsg::Send {
                    to_address: "treasury".to_string(),
                    amount: vec![Coin::new(250, "in_denom")],
                })
            );
            assert_eq!(
                res.messages[1].msg,
                cosmwasm_std::CosmosMsg::Bank(BankMsg::Send {
                    to_address: "collector".to_string(),
                    amount: vec![Coin::new(100, "fee")],
                })
            );
            assert_eq!(
                res.messages[2].msg,
                cosmwasm_std::CosmosMsg::Bank(BankMsg::Send {
                    to_address: "collector".to_string(),
                    amount: vec![Coin::new(2, "in_denom")],
                })
            )
        }

        #[test]
        fn test_threshold_not_reached() {
            let treasury = Addr::unchecked("treasury");
            let start = Timestamp::from_seconds(1_000_000);
            let end = Timestamp::from_seconds(5_000_000);
            let out_supply = Uint256::from(500u128);
            let out_denom = "out_denom";
            let in_denom = "in_denom";

            // threshold = 500*0.5 / 1-0.01 =252.5

            // instantiate
            let mut deps = mock_dependencies();
            let mut env = mock_env();
            env.block.height = 0;
            let msg = crate::msg::InstantiateMsg {
                min_stream_seconds: Uint64::new(1000),
                min_seconds_until_start_time: Uint64::new(0),
                stream_creation_denom: "fee".to_string(),
                stream_creation_fee: Uint128::new(100),
                exit_fee_percent: Decimal256::percent(1),
                fee_collector: "collector".to_string(),
                protocol_admin: "protocol_admin".to_string(),
                accepted_in_denom: in_denom.to_string(),
            };
            instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

            // create stream
            let mut env = mock_env();
            env.block.time = Timestamp::from_seconds(0);
            let info = mock_info(
                "creator",
                &[
                    Coin::new(out_supply.to_string().parse().unwrap(), out_denom),
                    Coin::new(100, "fee"),
                ],
            );
            execute_create_stream(
                deps.as_mut(),
                env,
                info,
                treasury.to_string(),
                "test".to_string(),
                Some("https://sample.url".to_string()),
                in_denom.to_string(),
                out_denom.to_string(),
                out_supply,
                start,
                end,
                Some(500u128.into()),
            )
            .unwrap();

            // Subscription 1
            let mut env = mock_env();
            env.block.time = start;
            let funds = Coin::new(250, "in_denom");
            let info = mock_info("subscriber", &[funds]);
            let msg = crate::msg::ExecuteMsg::Subscribe {
                stream_id: 1,
                operator_target: None,
                operator: Some("operator".to_string()),
            };
            let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

            // Subscription 2
            let funds = Coin::new(1, "in_denom");
            let info = mock_info("subscriber2", &[funds]);
            let msg = crate::msg::ExecuteMsg::Subscribe {
                stream_id: 1,
                operator_target: None,
                operator: Some("operator".to_string()),
            };
            let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

            // Set time to the end of the stream
            let mut env = mock_env();
            env.block.time = end.plus_seconds(1);

            // Exit should not be possible
            let info = mock_info("subscriber", &[]);
            let res = execute_exit_stream(deps.as_mut(), env.clone(), info, 1, None).unwrap_err();
            assert_eq!(
                res,
                ContractError::ThresholdError(ThresholdError::ThresholdNotReached {})
            );

            // Finalize should not be possible
            let info = mock_info("treasury", &[]);
            let res =
                execute_finalize_stream(deps.as_mut(), env.clone(), info, 1, None).unwrap_err();
            assert_eq!(
                res,
                ContractError::ThresholdError(ThresholdError::ThresholdNotReached {})
            );

            // Subscriber one executes exit cancelled before creator cancels stream
            let info = mock_info("subscriber", &[]);
            let res = execute_exit_cancelled(deps.as_mut(), env.clone(), info, 1, None).unwrap();
            assert_eq!(
                res.messages,
                vec![SubMsg::new(BankMsg::Send {
                    to_address: "subscriber".to_string(),
                    amount: vec![Coin::new(250, "in_denom")],
                })]
            );
            // Creator threshold cancels the stream
            let info = mock_info("treasury", &[]);
            let res =
                execute_cancel_stream_with_threshold(deps.as_mut(), env.clone(), info, 1).unwrap();
            assert_eq!(
                res.messages,
                vec![
                    // Out denom refunded
                    SubMsg::new(BankMsg::Send {
                        to_address: "treasury".to_string(),
                        amount: vec![Coin::new(500, "out_denom")],
                    }),
                ]
            );
            // Creator can not finalize the stream
            let info = mock_info("treasury", &[]);
            let res =
                execute_finalize_stream(deps.as_mut(), env.clone(), info, 1, None).unwrap_err();
            assert_eq!(res, ContractError::StreamKillswitchActive {});

            // Creator can not cancel the stream again
            let info = mock_info("treasury", &[]);
            let res = execute_cancel_stream_with_threshold(deps.as_mut(), env.clone(), info, 1)
                .unwrap_err();
            assert_eq!(res, ContractError::StreamKillswitchActive {});

            // Subscriber 2 executes exit cancelled after creator cancels stream
            let info = mock_info("subscriber2", &[]);
            let res = execute_exit_cancelled(deps.as_mut(), env.clone(), info, 1, None).unwrap();
            assert_eq!(
                // In denom refunded
                res.messages,
                vec![SubMsg::new(BankMsg::Send {
                    to_address: "subscriber2".to_string(),
                    amount: vec![Coin::new(1, "in_denom")],
                })]
            );
        }

        #[test]
        fn test_threshold_cancel() {
            let treasury = Addr::unchecked("treasury");
            let start = Timestamp::from_seconds(1_000_000);
            let end = Timestamp::from_seconds(5_000_000);
            let out_supply = Uint256::from(500u128);
            let out_denom = "out_denom";
            let in_denom = "in_denom";

            // threshold = 500*0.5 / 1-0.01 =252.5

            // instantiate
            let mut deps = mock_dependencies();
            let mut env = mock_env();
            env.block.time = Timestamp::from_seconds(0);
            let msg = crate::msg::InstantiateMsg {
                min_stream_seconds: Uint64::new(1000),
                min_seconds_until_start_time: Uint64::new(0),
                stream_creation_denom: "fee".to_string(),
                stream_creation_fee: Uint128::new(100),
                exit_fee_percent: Decimal256::percent(1),
                fee_collector: "collector".to_string(),
                protocol_admin: "protocol_admin".to_string(),
                accepted_in_denom: in_denom.to_string(),
            };
            instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

            // create stream
            let mut env = mock_env();
            env.block.time = Timestamp::from_seconds(0);
            let info = mock_info(
                "creator",
                &[
                    Coin::new(out_supply.to_string().parse().unwrap(), out_denom),
                    Coin::new(100, "fee"),
                ],
            );
            execute_create_stream(
                deps.as_mut(),
                env,
                info,
                treasury.to_string(),
                "test".to_string(),
                Some("https://sample.url".to_string()),
                in_denom.to_string(),
                out_denom.to_string(),
                out_supply,
                start,
                end,
                Some(1_000u128.into()),
            )
            .unwrap();

            // Subscription 1
            let mut env = mock_env();
            env.block.time = start;
            let funds = Coin::new(250, "in_denom");
            let info = mock_info("subscriber", &[funds]);
            let msg = crate::msg::ExecuteMsg::Subscribe {
                stream_id: 1,
                operator_target: None,
                operator: Some("operator".to_string()),
            };
            let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

            // Subscription 2
            let funds = Coin::new(500, "in_denom");
            let info = mock_info("subscriber2", &[funds]);
            let msg = crate::msg::ExecuteMsg::Subscribe {
                stream_id: 1,
                operator_target: None,
                operator: Some("operator".to_string()),
            };
            let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();
            // Can not cancel stream before it ends
            let mut env = mock_env();
            env.block.time = start.plus_seconds(1_000_000);
            let res = execute_cancel_stream_with_threshold(
                deps.as_mut(),
                env,
                mock_info("treasury", &[]),
                1,
            )
            .unwrap_err();
            assert_eq!(res, ContractError::StreamNotEnded {});

            // Set block to the end of the stream
            let mut env = mock_env();
            env.block.time = end.plus_seconds(1);

            // Non creator can't cancel stream
            let res = execute_cancel_stream_with_threshold(
                deps.as_mut(),
                env.clone(),
                mock_info("random", &[]),
                1,
            )
            .unwrap_err();
            assert_eq!(res, ContractError::Unauthorized {});

            // Creator can cancel stream
            let _res = execute_cancel_stream_with_threshold(
                deps.as_mut(),
                env.clone(),
                mock_info("treasury", &[]),
                1,
            )
            .unwrap();
            // Query stream should return stream with is_cancelled = true
            let stream = query_stream(deps.as_ref(), env.clone(), 1).unwrap();
            assert_eq!(stream.status, Status::Cancelled);
        }
    }
}
