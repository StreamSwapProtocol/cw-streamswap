use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Coin, Decimal};

#[cw_serde]
pub struct Params {
    pub protocol_admin: Addr,
    pub fee_collector: Addr,
    pub stream_creation_fee: Coin,
    pub exit_fee_percent: Decimal,
    pub stream_swap_code_id: u64,
    pub vesting_code_id: u64,
    pub accepted_in_denoms: Vec<String>,
    pub min_stream_seconds: u64,
    pub min_seconds_until_start_time: u64,
}
