use super::setup::TestAccounts;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{coin, Coin, Decimal, Timestamp, Uint128};
use cw_vesting::msg::InstantiateMsg as VestingInstantiateMsg;
use streamswap_factory::msg::{
    CreatePool, CreateStreamMsg, ExecuteMsg as FactoryExecuteMsg,
    InstantiateMsg as FactoryInstantiateMsg,
};

pub fn get_factory_inst_msg(
    stream_swap_code_id: u64,
    vesting_code_id: u64,
    test_accounts: &TestAccounts,
) -> FactoryInstantiateMsg {
    FactoryInstantiateMsg {
        stream_swap_code_id,
        vesting_code_id,
        protocol_admin: Some(test_accounts.admin.to_string()),
        fee_collector: Some(test_accounts.admin.to_string()),
        stream_creation_fee: Coin {
            denom: "fee_denom".to_string(),
            amount: 100u128.into(),
        },
        exit_fee_percent: Decimal::percent(1),
        accepted_in_denoms: vec!["in_denom".to_string()],
        min_stream_seconds: 100,
        min_seconds_until_start_time: 100,
        pool_creation_denom: "uosmo".to_string(),
    }
}

// TODO: explore using builder for messages
/*
pub struct CreateStreamMsgBuilder {
    treasury: String,
    stream_admin: String,
    name: String,
    url: Option<String>,
    out_asset: Coin,
    in_denom: String,
    start_time: Timestamp,
    end_time: Timestamp,
    threshold: Option<Uint128>,
    create_pool: Option<CreatePool>,
    vesting: Option<VestingInstantiateMsg>,
}

impl CreateStreamMsgBuilder {
    // Creates a builder with default values
    pub fn new() -> Self {
        CreateStreamMsgBuilder {
            treasury: "treasury".to_string(),
            stream_admin: "admin".to_string(),
            name: "stream 1".to_string(),
            url: None,
            out_asset: coin(100, "out_denom".to_string()),
            in_denom: "in_denom".to_string(),
            start_time: Timestamp::from_seconds(100),
            end_time: Timestamp::from_seconds(200),
            threshold: None,
            create_pool: None,
            vesting: None,
        }
    }
    pub fn with_treasury(mut self, treasury: String) -> Self {
        self.treasury = treasury;
        self
    }

    pub fn with_stream_admin(mut self, stream_admin: String) -> Self {
        self.stream_admin = stream_admin;
        self
    }

    pub fn with_start_time(mut self, start_time: Timestamp) -> Self {
        self.start_time = start_time;
        self
    }

    pub fn with_end_time(mut self, end_time: Timestamp) -> Self {
        self.end_time = end_time;
        self
    }

    pub fn with_out_asset(mut self, out_asset: Coin) -> Self {
        self.out_asset = out_asset;
        self
    }

    pub fn with_in_denom(mut self, in_denom: String) -> Self {
        self.in_denom = in_denom;
        self
    }

    pub fn with_url(mut self, url: String) -> Self {
        self.url = Some(url);
        self
    }

    pub fn with_threshold(mut self, threshold: Uint128) -> Self {
        self.threshold = Some(threshold);
        self
    }

    pub fn with_create_pool(mut self, create_pool: CreatePool) -> Self {
        self.create_pool = Some(create_pool);
        self
    }

    pub fn with_vesting(mut self, vesting: VestingInstantiateMsg) -> Self {
        self.vesting = Some(vesting);
        self
    }

    pub fn build(self) -> CreateStreamMsg {
        CreateStreamMsg {
            treasury: self.treasury,
            stream_admin: self.stream_admin,
            name: self.name,
            url: self.url,
            out_asset: self.out_asset,
            in_denom: self.in_denom,
            start_time: self.start_time,
            end_time: self.end_time,
            threshold: self.threshold,
            create_pool: self.create_pool,
            vesting: self.vesting,
        }
    }
}

 */

pub fn get_create_stream_msg(
    name: &str,
    url: Option<String>,
    treasury: &str,
    out_asset: Coin,
    in_denom: &str,
    start_time: Timestamp,
    end_time: Timestamp,
    threshold: Option<Uint128>,
    create_pool: Option<CreatePool>,
    vesting: Option<VestingInstantiateMsg>,
) -> FactoryExecuteMsg {
    FactoryExecuteMsg::CreateStream {
        msg: CreateStreamMsg {
            treasury: treasury.to_string(),
            stream_admin: treasury.to_string(),
            name: name.to_string(),
            url,
            out_asset,
            in_denom: in_denom.to_string(),
            start_time,
            end_time,
            threshold,
            create_pool,
            vesting,
        },
    }
}
