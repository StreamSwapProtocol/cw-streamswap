use cosmwasm_std::testing::MockApi;
use cosmwasm_std::{coin, Addr, BlockInfo, Coin, Timestamp};
use cw_multi_test::addons::{MockAddressGenerator, MockApiBech32};
use cw_multi_test::{
    no_init, App, AppBuilder, BankKeeper, BankSudo, ContractWrapper, Executor, SudoMsg, WasmKeeper,
};
use streamswap_factory::contract::{
    execute as factory_execute, instantiate as factory_instantiate, query as factory_query,
};
use streamswap_stream::contract::{
    execute as streamswap_execute, instantiate as streamswap_instantiate, query as streamswap_query,
};

pub const PREFIX: &str = "cosmwasm";

pub fn setup() -> SetupResponse {
    let accounts = create_test_accounts();
    let denoms = vec![
        "fee_denom".to_string(),
        "out_denom".to_string(),
        "in_denom".to_string(),
        "wrong_denom".to_string(),
    ];
    let amount = 1_000_000_000_000_000u128;
    let mut app = AppBuilder::default()
        .with_api(MockApiBech32::new(PREFIX))
        .with_wasm(WasmKeeper::default().with_address_generator(MockAddressGenerator))
        .build(|router, api, storage| {
            accounts.all().iter().for_each(|account| {
                let coins = denoms.iter().map(|d| coin(amount, d.clone())).collect();
                router.bank.init_balance(storage, account, coins).unwrap();
            });
        });

    app.set_block(BlockInfo {
        chain_id: "test_1".to_string(),
        height: 1_000,
        time: Timestamp::from_seconds(1_000),
    });

    let stream_swap_factory_contract = Box::new(ContractWrapper::new(
        factory_execute,
        factory_instantiate,
        factory_query,
    ));
    let stream_swap_contract = Box::new(ContractWrapper::new(
        streamswap_execute,
        streamswap_instantiate,
        streamswap_query,
    ));
    let vesting_contract = Box::new(ContractWrapper::new(
        cw_vesting::contract::execute,
        cw_vesting::contract::instantiate,
        cw_vesting::contract::query,
    ));

    let stream_swap_code_id = app.store_code(stream_swap_contract);
    let stream_swap_factory_code_id = app.store_code(stream_swap_factory_contract);
    let vesting_code_id = app.store_code(vesting_contract);

    SetupResponse {
        test_accounts: accounts,
        stream_swap_factory_code_id,
        stream_swap_code_id,
        vesting_code_id,
        app,
    }
}

fn create_test_accounts() -> TestAccounts {
    let admin = Addr::unchecked("cosmwasm1txtvsrrlxjx6w8u0txlkyrgr5pryppy0qxurhf");
    let creator_1 = Addr::unchecked("cosmwasm1cr3y8u3e4s8cvdcmzsc3npamqnlrfm3laq5knl");
    let subscriber_1 = Addr::unchecked("cosmwasm1a3tg0fs480c2lgv3ter6gr48rvs44y5gyxs6fc");
    let subscriber_2 = Addr::unchecked("cosmwasm1x59j93fhlmu3hvr62seczznmjfhpgcfm8ytjhk");
    let wrong_user = Addr::unchecked("cosmwasm1g9ezj6tasvnvxx4y9a7mv2k4pzl57dzs0s6k5q");
    let creator_2 = Addr::unchecked("cosmwasm13j0qnl00r0rl42mezg3ntc3syzaswgnvrzlmx6");

    TestAccounts {
        admin,
        creator_1,
        subscriber_1,
        subscriber_2,
        wrong_user,
        creator_2,
    }
}

pub struct SetupResponse {
    pub app: App<BankKeeper, MockApiBech32>,
    pub test_accounts: TestAccounts,
    pub stream_swap_factory_code_id: u64,
    pub stream_swap_code_id: u64,
    pub vesting_code_id: u64,
}

pub struct TestAccounts {
    pub admin: Addr,
    pub creator_1: Addr,
    pub subscriber_1: Addr,
    pub subscriber_2: Addr,
    pub wrong_user: Addr,
    pub creator_2: Addr,
}
impl TestAccounts {
    pub fn all(&self) -> Vec<Addr> {
        vec![
            self.admin.clone(),
            self.creator_1.clone(),
            self.subscriber_1.clone(),
            self.subscriber_2.clone(),
            self.wrong_user.clone(),
            self.creator_2.clone(),
        ]
    }
}
