use crate::ContractError;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Decimal256, Storage, Timestamp, Uint128, Uint256, Uint64};
use cw_storage_plus::{Item, Map};
use std::ops::Mul;

#[cw_serde]
pub struct Config {
    /// Minimum sale duration in unix seconds
    pub min_stream_seconds: Uint64,
    /// Minimum duration between start time and current time in unix seconds
    pub min_seconds_until_start_time: Uint64,
    /// Accepted in_denom to buy out_tokens
    pub accepted_in_denom: String,
    /// Accepted stream creation fee denom
    pub stream_creation_denom: String,
    /// Stream creation fee amount
    pub stream_creation_fee: Uint128,
    /// in/buy token exit fee in percent
    pub exit_fee_percent: Decimal256,
    /// Address of the fee collector
    pub fee_collector: Addr,
    /// protocol admin can pause streams in case of emergency.
    pub protocol_admin: Addr,
}

pub const CONFIG: Item<Config> = Item::new("config");

#[cw_serde]
pub struct Stream {
    /// Name of the stream.
    pub name: String,
    /// Destination for the earned token_in.
    pub treasury: Addr,
    /// URL for more information about the stream.
    pub url: Option<String>,
    /// Proportional distribution variable to calculate the distribution of in token_out to buyers.
    pub dist_index: Decimal256,
    /// last updated time of stream.
    pub last_updated: Timestamp,
    /// denom of the `token_out`.
    pub out_denom: String,
    /// total number of `token_out` to be sold during the continuous stream.
    pub out_supply: Uint256,
    /// total number of remaining out tokens at the time of update.
    pub out_remaining: Uint256,
    /// denom of the `token_in`.
    pub in_denom: String,
    /// total number of `token_in` on the buy side at latest state.
    pub in_supply: Uint256,
    /// total number of `token_in` spent at latest state.
    pub spent_in: Uint256,
    /// total number of shares minted.
    pub shares: Uint256,
    /// start time when the token emission starts. in nanos.
    pub start_time: Timestamp,
    /// end time when the token emission ends.
    pub end_time: Timestamp,
    /// price at when latest distribution is triggered.
    pub current_streamed_price: Decimal256,
    /// Status of the stream. Can be `Waiting`, `Active`, `Finalized`, `Paused` or `Canceled` for kill switch.
    pub status: Status,
    /// Date when the stream was paused.
    pub pause_date: Option<Timestamp>,
    /// Stream creation fee denom. Saved under here to avoid any changes in config to efect existing streams.
    pub stream_creation_denom: String,
    /// Stream creation fee amount. Saved under here to avoid any changes in config to efect existing streams.
    pub stream_creation_fee: Uint128,
    /// Stream swap fee in percent. Saved under here to avoid any changes in config to efect existing streams.
    pub stream_exit_fee_percent: Decimal256,
}

#[cw_serde]
pub enum Status {
    /// Waiting for start date
    Waiting,
    Active,
    Finalized,
    Paused,
    Cancelled,
}
#[allow(clippy::too_many_arguments)]
impl Stream {
    pub fn new(
        name: String,
        treasury: Addr,
        url: Option<String>,
        out_denom: String,
        out_supply: Uint256,
        in_denom: String,
        start_time: Timestamp,
        end_time: Timestamp,
        last_updated: Timestamp,
        stream_creation_denom: String,
        stream_creation_fee: Uint128,
        stream_exit_fee_percent: Decimal256,
    ) -> Self {
        Stream {
            name,
            treasury,
            url,
            dist_index: Decimal256::zero(),
            last_updated,
            out_denom,
            out_supply,
            out_remaining: out_supply,
            in_denom,
            in_supply: Uint256::zero(),
            spent_in: Uint256::zero(),
            shares: Uint256::zero(),
            start_time,
            end_time,
            current_streamed_price: Decimal256::zero(),
            status: Status::Waiting,
            pause_date: None,
            stream_creation_denom,
            stream_creation_fee,
            stream_exit_fee_percent,
        }
    }

    // compute amount of shares that should be minted for a new subscription amount
    pub fn compute_shares_amount(&self, amount_in: Uint256, round_up: bool) -> Uint256 {
        if self.shares.is_zero() || amount_in.is_zero() {
            return amount_in.into();
        }
        let mut shares = self.shares.mul(amount_in);
        if round_up {
            shares = (shares + self.in_supply - Uint256::one()) / self.in_supply;
        } else {
            shares /= self.in_supply
        }
        shares
    }

    pub fn is_paused(&self) -> bool {
        self.status == Status::Paused
    }

    pub fn is_cancelled(&self) -> bool {
        self.status == Status::Cancelled
    }

    pub fn is_killswitch_active(&self) -> bool {
        self.status == Status::Cancelled || self.status == Status::Paused
    }
}
pub type StreamId = u64;
pub const STREAMS: Map<StreamId, Stream> = Map::new("stream");
const STREAM_ID_COUNTER: Item<StreamId> = Item::new("stream_id_counter");
pub fn next_stream_id(store: &mut dyn Storage) -> Result<u64, ContractError> {
    let id: u64 = STREAM_ID_COUNTER.may_load(store)?.unwrap_or_default() + 1;
    STREAM_ID_COUNTER.save(store, &id)?;
    Ok(id)
}

#[cw_serde]
pub struct Position {
    /// creator of the position.
    pub owner: Addr,
    /// current amount of tokens in buy pool
    pub in_balance: Uint256,
    pub shares: Uint256,
    // index is used to calculate the distribution a position has
    pub index: Decimal256,
    pub last_updated: Timestamp,
    // total amount of `token_out` purchased in tokens at latest calculation
    pub purchased: Uint256,
    // pending purchased accumulates purchases after decimal truncation
    pub pending_purchase: Decimal256,
    // total amount of `token_in` spent tokens at latest calculation
    pub spent: Uint256,
    // operator can update position
    pub operator: Option<Addr>,
}

impl Position {
    pub fn new(
        owner: Addr,
        in_balance: Uint256,
        shares: Uint256,
        index: Option<Decimal256>,
        last_updated: Timestamp,
        operator: Option<Addr>,
    ) -> Self {
        Position {
            owner,
            in_balance,
            shares,
            index: index.unwrap_or_default(),
            last_updated,
            purchased: Uint256::zero(),
            pending_purchase: Decimal256::zero(),
            spent: Uint256::zero(),
            operator,
        }
    }
}

// Position (stream_id, owner_addr) -> Position
pub const POSITIONS: Map<(StreamId, &Addr), Position> = Map::new("positions");

// Testing module
#[cfg(test)]

mod tests {
    use super::*;
    use cosmwasm_std::{Addr, Uint128};

    // Test compute_shares_amount
    #[test]
    fn test_compute_shares_amount() {
        let stream = Stream {
            name: "test".to_string(),
            treasury: Addr::unchecked("Addr"),
            url: None,
            dist_index: Decimal256::zero(),
            last_updated: Timestamp::from_nanos(1722006000000000000),
            out_denom: "out_denom".to_string(),
            out_supply: Uint256::from(79000000000u128),
            out_remaining: Uint256::from(79000000000u128),
            in_denom: "in_denom".to_string(),
            in_supply: Uint256::from(312458028446265623240u128),
            spent_in: Uint256::zero(),
            shares: Uint256::from(312458028446265623240u128),
            start_time: Timestamp::from_nanos(1722006000000000000),
            end_time: Timestamp::from_nanos(1722009600000000000),
            current_streamed_price: Decimal256::zero(),
            status: Status::Waiting,
            pause_date: None,
            stream_creation_denom: "fee_denom".to_string(),
            stream_creation_fee: Uint128::from(150000000000000000000u128),
            stream_exit_fee_percent: Decimal256::percent(1),
        };

        // Test when shares is zero
        let shares = stream.compute_shares_amount(Uint256::from(2000000000000000000u128), false);
        assert_eq!(shares, Uint256::from(2000000000000000000u128));
    }
}
