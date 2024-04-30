#![cfg(test)]
use crate::helpers::{
    mock_messages::{get_create_stream_msg, get_factory_inst_msg},
    setup::{setup, SetupResponse},
};
use cosmwasm_std::{coin, Addr, Decimal};
use cw_multi_test::Executor;
use cw_streamswap_factory::error::ContractError as FactoryError;
use cw_streamswap_factory::{msg::QueryMsg, state::Params};

#[test]
fn factory_freeze() {
    let SetupResponse {
        mut app,
        test_accounts,
        stream_swap_code_id,
        stream_swap_factory_code_id,
    } = setup();

    let msg = get_factory_inst_msg(stream_swap_code_id, &test_accounts);
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
        &test_accounts.creator.to_string(),
        coin(100, "out_denom"),
        "in_denom",
        app.block_info().height + 100,
        app.block_info().height + 200,
        None,
    );
    let _create_stream_res = app
        .execute_contract(
            test_accounts.creator.clone(),
            factory_address.clone(),
            &create_stream_msg,
            &[coin(100, "fee_token"), coin(100, "out_denom")],
        )
        .unwrap();

    // Non-admin cannot freeze factory
    let msg = cw_streamswap_factory::msg::ExecuteMsg::Freeze {};
    let res = app
        .execute_contract(
            test_accounts.subscriber.clone(),
            factory_address.clone(),
            &msg,
            &[],
        )
        .unwrap_err();
    let err = res.source().unwrap();
    let error = err.downcast_ref::<FactoryError>().unwrap();
    assert_eq!(*error, FactoryError::Unauthorized {});

    // Admin can freeze factory
    let msg = cw_streamswap_factory::msg::ExecuteMsg::Freeze {};
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
        &test_accounts.creator.to_string(),
        coin(100, "out_denom"),
        "in_denom",
        app.block_info().height + 100,
        app.block_info().height + 200,
        None,
    );
    let res = app
        .execute_contract(
            test_accounts.creator.clone(),
            factory_address.clone(),
            &create_stream_msg,
            &[coin(100, "fee_token"), coin(100, "out_denom")],
        )
        .unwrap_err();
    let err = res.source().unwrap();
    let error = err.downcast_ref::<FactoryError>().unwrap();
    assert_eq!(*error, FactoryError::ContractIsFrozen {});
}
