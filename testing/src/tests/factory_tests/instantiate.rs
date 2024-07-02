#![cfg(test)]
use crate::helpers::{
    mock_messages::get_factory_inst_msg,
    setup::{setup, SetupResponse},
};
use cosmwasm_std::{coin, Decimal};
use cw_multi_test::Executor;
use streamswap_factory::{msg::QueryMsg, state::Params};

#[test]
fn factory_proper_instantiate() {
    //let mut setup_res = setup();
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

    // Query Params
    let res: Params = app
        .wrap()
        .query_wasm_smart(factory_address, &QueryMsg::Params {})
        .unwrap();
    assert_eq!(res.stream_creation_fee, coin(100, "fee_denom"));
    assert_eq!(res.exit_fee_percent, Decimal::percent(1));
    assert_eq!(res.stream_swap_code_id, stream_swap_code_id);
    assert_eq!(res.accepted_in_denoms, vec!["in_denom".to_string()]);
    assert_eq!(res.fee_collector, test_accounts.admin.to_string());
    assert_eq!(res.min_stream_seconds, 100);
    assert_eq!(res.min_seconds_until_start_time, 100);
}
