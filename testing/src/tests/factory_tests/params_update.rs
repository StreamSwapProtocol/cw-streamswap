#![cfg(test)]
use crate::helpers::{
    mock_messages::get_factory_inst_msg,
    setup::{setup, SetupResponse},
};
use cosmwasm_std::{coin, Addr, Decimal};
use cw_multi_test::Executor;
use cw_streamswap_factory::error::ContractError as FactoryError;
use cw_streamswap_factory::{msg::QueryMsg, state::Params};

#[test]
fn params_update() {
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

    // Non-admin cannot update params
    let msg = cw_streamswap_factory::msg::ExecuteMsg::UpdateParams {
        stream_creation_fee: Some(coin(100, "fee_token")),
        exit_fee_percent: None,
        accepted_in_denoms: None,
        fee_collector: None,
        min_stream_blocks: None,
        min_blocks_until_start_block: None,
    };
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

    // Update stream creation fee
    let msg = cw_streamswap_factory::msg::ExecuteMsg::UpdateParams {
        stream_creation_fee: Some(coin(200, "fee_token")),
        exit_fee_percent: None,
        accepted_in_denoms: None,
        fee_collector: None,
        min_stream_blocks: None,
        min_blocks_until_start_block: None,
    };
    let _ = app
        .execute_contract(
            test_accounts.admin.clone(),
            factory_address.clone(),
            &msg,
            &[],
        )
        .unwrap();

    // Query Params
    let res: Params = app
        .wrap()
        .query_wasm_smart(factory_address.clone(), &QueryMsg::Params {})
        .unwrap();

    assert_eq!(res.stream_creation_fee, coin(200, "fee_token"));

    // Update wrong exit fee percent
    let msg = cw_streamswap_factory::msg::ExecuteMsg::UpdateParams {
        stream_creation_fee: None,
        exit_fee_percent: Some(Decimal::percent(101)),
        accepted_in_denoms: None,
        fee_collector: None,
        min_stream_blocks: None,
        min_blocks_until_start_block: None,
    };
    let res = app
        .execute_contract(
            test_accounts.admin.clone(),
            factory_address.clone(),
            &msg,
            &[],
        )
        .unwrap_err();
    let err = res.source().unwrap();
    let error = err.downcast_ref::<FactoryError>().unwrap();
    assert_eq!(*error, FactoryError::InvalidExitFeePercent {});

    // Update exit fee percent
    let msg = cw_streamswap_factory::msg::ExecuteMsg::UpdateParams {
        stream_creation_fee: None,
        exit_fee_percent: Some(Decimal::percent(50)),
        accepted_in_denoms: None,
        fee_collector: None,
        min_stream_blocks: None,
        min_blocks_until_start_block: None,
    };
    let _ = app
        .execute_contract(
            test_accounts.admin.clone(),
            factory_address.clone(),
            &msg,
            &[],
        )
        .unwrap();

    // Query Params
    let res: Params = app
        .wrap()
        .query_wasm_smart(factory_address.clone(), &QueryMsg::Params {})
        .unwrap();

    assert_eq!(res.exit_fee_percent, Decimal::percent(50));

    // Update accepted in denoms
    let msg = cw_streamswap_factory::msg::ExecuteMsg::UpdateParams {
        stream_creation_fee: None,
        exit_fee_percent: None,
        accepted_in_denoms: Some(vec!["denom1".to_string(), "denom2".to_string()]),
        fee_collector: None,
        min_stream_blocks: None,
        min_blocks_until_start_block: None,
    };
    let _ = app
        .execute_contract(
            test_accounts.admin.clone(),
            factory_address.clone(),
            &msg,
            &[],
        )
        .unwrap();

    // Query Params
    let res: Params = app
        .wrap()
        .query_wasm_smart(factory_address.clone(), &QueryMsg::Params {})
        .unwrap();

    assert_eq!(
        res.accepted_in_denoms,
        vec!["denom1".to_string(), "denom2".to_string()]
    );

    // Update fee collector
    let msg = cw_streamswap_factory::msg::ExecuteMsg::UpdateParams {
        stream_creation_fee: None,
        exit_fee_percent: None,
        accepted_in_denoms: None,
        fee_collector: Some("new_fee_collector".to_string()),
        min_stream_blocks: None,
        min_blocks_until_start_block: None,
    };
    let _ = app
        .execute_contract(
            test_accounts.admin.clone(),
            factory_address.clone(),
            &msg,
            &[],
        )
        .unwrap();

    // Query Params
    let res: Params = app
        .wrap()
        .query_wasm_smart(factory_address.clone(), &QueryMsg::Params {})
        .unwrap();

    assert_eq!(res.fee_collector, Addr::unchecked("new_fee_collector"));

    // Update min stream blocks
    let msg = cw_streamswap_factory::msg::ExecuteMsg::UpdateParams {
        stream_creation_fee: None,
        exit_fee_percent: None,
        accepted_in_denoms: None,
        fee_collector: None,
        min_stream_blocks: Some(100),
        min_blocks_until_start_block: None,
    };
    let _ = app
        .execute_contract(
            test_accounts.admin.clone(),
            factory_address.clone(),
            &msg,
            &[],
        )
        .unwrap();

    // Query Params
    let res: Params = app
        .wrap()
        .query_wasm_smart(factory_address.clone(), &QueryMsg::Params {})
        .unwrap();

    assert_eq!(res.min_stream_blocks, 100);

    // Update min blocks until start block
    let msg = cw_streamswap_factory::msg::ExecuteMsg::UpdateParams {
        stream_creation_fee: None,
        exit_fee_percent: None,
        accepted_in_denoms: None,
        fee_collector: None,
        min_stream_blocks: None,
        min_blocks_until_start_block: Some(100),
    };
    let _ = app
        .execute_contract(
            test_accounts.admin.clone(),
            factory_address.clone(),
            &msg,
            &[],
        )
        .unwrap();

    // Query Params
    let res: Params = app
        .wrap()
        .query_wasm_smart(factory_address.clone(), &QueryMsg::Params {})
        .unwrap();

    assert_eq!(res.min_blocks_until_start_block, 100);
}