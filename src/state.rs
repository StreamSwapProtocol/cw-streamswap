use crate::{msg::ChainHaltResponse, ContractError};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    Addr, BlockInfo, Decimal, Decimal256, Response, Storage, Timestamp, Uint128, Uint64,
};
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
    pub exit_fee_percent: Decimal,
    /// Address of the fee collector
    pub fee_collector: Addr,
    /// protocol admin can pause streams in case of emergency.
    pub protocol_admin: Addr,
    /// Avarage block time in nanoseconds for detecting any chain halt
    pub average_block_time: u64,
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
    pub last_updated_time: Timestamp,
    /// last updated block of stream.
    pub last_updated_block: u64,
    /// denom of the `token_out`.
    pub out_denom: String,
    /// total number of `token_out` to be sold during the continuous stream.
    pub out_supply: Uint128,
    /// total number of remaining out tokens at the time of update.
    pub out_remaining: Uint128,
    /// denom of the `token_in`.
    pub in_denom: String,
    /// total number of `token_in` on the buy side at latest state.
    pub in_supply: Uint128,
    /// total number of `token_in` spent at latest state.
    pub spent_in: Uint128,
    /// total number of shares minted.
    pub shares: Uint128,
    /// start time when the token emission starts. in nanos.
    pub start_time: Timestamp,
    /// end time when the token emission ends.
    pub end_time: Timestamp,
    /// price at when latest distribution is triggered.
    pub current_streamed_price: Decimal,
    /// Status of the stream. Can be `Waiting`, `Active`, `Finalized`, `Paused` or `Canceled` for kill switch.
    pub status: Status,
    /// Date when the stream was paused.
    pub pause_date: Option<Timestamp>,
    /// Stream creation fee denom. Saved under here to avoid any changes in config to efect existing streams.
    pub stream_creation_denom: String,
    /// Stream creation fee amount. Saved under here to avoid any changes in config to efect existing streams.
    pub stream_creation_fee: Uint128,
    /// Stream swap fee in percent. Saved under here to avoid any changes in config to efect existing streams.
    pub stream_exit_fee_percent: Decimal,
}

#[cw_serde]
pub enum Status {
    /// Waiting for start date
    Waiting,
    Active,
    Finalized,
    Paused,
    Cancelled,
    ChainHalted,
}
#[allow(clippy::too_many_arguments)]
impl Stream {
    pub fn new(
        name: String,
        treasury: Addr,
        url: Option<String>,
        out_denom: String,
        out_supply: Uint128,
        in_denom: String,
        start_time: Timestamp,
        end_time: Timestamp,
        last_updated_time: Timestamp,
        last_updated_block: u64,
        stream_creation_denom: String,
        stream_creation_fee: Uint128,
        stream_exit_fee_percent: Decimal,
    ) -> Self {
        Stream {
            name,
            treasury,
            url,
            dist_index: Decimal256::zero(),
            last_updated_time: last_updated_time,
            last_updated_block: last_updated_block,
            out_denom,
            out_supply,
            out_remaining: out_supply,
            in_denom,
            in_supply: Uint128::zero(),
            spent_in: Uint128::zero(),
            shares: Uint128::zero(),
            start_time,
            end_time,
            current_streamed_price: Decimal::zero(),
            status: Status::Waiting,
            pause_date: None,
            stream_creation_denom,
            stream_creation_fee,
            stream_exit_fee_percent,
        }
    }
    pub fn detect_chain_halt(
        &self,
        current_block_info: BlockInfo,
        average_block_time: u64,
    ) -> Option<Response> {
        // Calculate how much time is passed
        let time_passed = current_block_info
            .time
            .nanos()
            .checked_sub(self.last_updated_time.nanos());
        // Calculate how many blocks are passed
        let blocks_passed = current_block_info
            .height
            .checked_sub(self.last_updated_block);
        // Calculate average block time
        let calculated_average_block_time: u64 = time_passed
            .unwrap()
            .checked_div(blocks_passed.unwrap())
            .unwrap_or(average_block_time);

        // Our average block time must be +- 10% of the config average block time
        if calculated_average_block_time > average_block_time * 110 / 100
            || calculated_average_block_time < average_block_time * 90 / 100
        {
            return Some(
                Response::new()
                    .add_attribute("action", "update_stream")
                    .add_attribute("status", "halted")
                    .add_attribute(
                        "calculated_average_block_time",
                        calculated_average_block_time.to_string(),
                    )
                    .add_attribute("chain_average_block_time", average_block_time.to_string())
                    .add_attribute("chain_block_height", current_block_info.height.to_string())
                    .add_attribute("chain_block_time", current_block_info.time.to_string())
                    .add_attribute("last_updated_block", self.last_updated_block.to_string())
                    .add_attribute("last_updated_time", self.last_updated_time.to_string()),
            );
        }
        None
    }

    // compute amount of shares that should be minted for a new subscription amount
    pub fn compute_shares_amount(&self, amount_in: Uint128, round_up: bool) -> Uint128 {
        if self.shares.is_zero() || amount_in.is_zero() {
            return amount_in;
        }
        let mut shares = self.shares.mul(amount_in);
        if round_up {
            shares = (shares + self.in_supply - Uint128::one()) / self.in_supply;
        } else {
            shares /= self.in_supply;
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
type StreamId = u64;
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
    pub in_balance: Uint128,
    pub shares: Uint128,
    // index is used to calculate the distribution a position has
    pub index: Decimal256,
    pub last_updated: Timestamp,
    // total amount of `token_out` purchased in tokens at latest calculation
    pub purchased: Uint128,
    // pending purchased accumulates purchases after decimal truncation
    pub pending_purchase: Decimal256,
    // total amount of `token_in` spent tokens at latest calculation
    pub spent: Uint128,
    // operator can update position
    pub operator: Option<Addr>,
}

impl Position {
    pub fn new(
        owner: Addr,
        in_balance: Uint128,
        shares: Uint128,
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
            purchased: Uint128::zero(),
            pending_purchase: Decimal256::zero(),
            spent: Uint128::zero(),
            operator,
        }
    }
}

// Position (stream_id, owner_addr) -> Position
pub const POSITIONS: Map<(StreamId, &Addr), Position> = Map::new("positions");
