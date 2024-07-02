#![cfg(test)]
use crate::helpers::{
    mock_messages::{get_create_stream_msg, get_factory_inst_msg},
    setup::{setup, SetupResponse},
};
use cosmwasm_std::coin;
use cw_multi_test::Executor;
use streamswap_factory::error::ContractError as FactoryError;
use streamswap_factory::msg::QueryMsg;

#[test]
fn factory_freeze() {
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
    // When factory is created, it is not frozen, Stream creation is allowed
    let create_stream_msg = get_create_stream_msg(
        "stream",
        None,
        &test_accounts.creator_1.to_string(),
        coin(100, "out_denom"),
        "in_denom",
        app.block_info().time.plus_seconds(100),
        app.block_info().time.plus_seconds(200),
        None,
        None,
        None,
    );
    let _create_stream_res = app
        .execute_contract(
            test_accounts.creator_1.clone(),
            factory_address.clone(),
            &create_stream_msg,
            &[coin(100, "fee_denom"), coin(100, "out_denom")],
        )
        .unwrap();

    // Non-admin cannot freeze factory
    let msg = streamswap_factory::msg::ExecuteMsg::Freeze {};
    let res = app
        .execute_contract(
            test_accounts.subscriber_1.clone(),
            factory_address.clone(),
            &msg,
            &[],
        )
        .unwrap_err();
    let err = res.source().unwrap();
    let error = err.downcast_ref::<FactoryError>().unwrap();
    assert_eq!(*error, FactoryError::Unauthorized {});

    // Admin can freeze factory
    let msg = streamswap_factory::msg::ExecuteMsg::Freeze {};
    app.execute_contract(
        test_accounts.admin.clone(),
        factory_address.clone(),
        &msg,
        &[],
    )
    .unwrap();

    // Query Params
    let res: bool = app
        .wrap()
        .query_wasm_smart(factory_address.clone(), &QueryMsg::Freezestate {})
        .unwrap();
    assert_eq!(res, true);

    // When factory is frozen, Stream creation is not allowed
    let create_stream_msg = get_create_stream_msg(
        "stream",
        None,
        &test_accounts.creator_1.to_string(),
        coin(100, "out_denom"),
        "in_denom",
        app.block_info().time.plus_seconds(100),
        app.block_info().time.plus_seconds(200),
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
    assert_eq!(*error, FactoryError::ContractIsFrozen {});
}
