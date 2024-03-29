use crate::ContractError;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Decimal, Decimal256, Storage, Uint128};
use cw_storage_plus::{Item, Map};
use std::ops::Mul;

#[cw_serde]
pub struct Config {
    /// Minimum sale duration as blocks
    pub min_stream_blocks: u64,
    /// Minimum duration between start_block and current_block
    pub min_blocks_until_start_block: u64,
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
    /// start block when the token emission starts. in nanos.
    pub start_block: u64,
    /// end block when the token emission ends.
    pub end_block: u64,
    /// price at when latest distribution is triggered.
    pub current_streamed_price: Decimal,
    /// Status of the stream. Can be `Waiting`, `Active`, `Finalized`, `Paused` or `Canceled` for kill switch.
    pub status: Status,
    /// Block height when the stream was paused.
    pub pause_block: Option<u64>,
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
        start_block: u64,
        end_block: u64,
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
            last_updated_block: last_updated_block,
            out_denom,
            out_supply,
            out_remaining: out_supply,
            in_denom,
            in_supply: Uint128::zero(),
            spent_in: Uint128::zero(),
            shares: Uint128::zero(),
            start_block,
            end_block,
            current_streamed_price: Decimal::zero(),
            status: Status::Waiting,
            pause_block: None,
            stream_creation_denom,
            stream_creation_fee,
            stream_exit_fee_percent,
        }
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
    // block height when the position was last updated.
    pub last_updated_block: u64,
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
        last_updated_block: u64,
        operator: Option<Addr>,
    ) -> Self {
        Position {
            owner,
            in_balance,
            shares,
            index: index.unwrap_or_default(),
            last_updated_block,
            purchased: Uint128::zero(),
            pending_purchase: Decimal256::zero(),
            spent: Uint128::zero(),
            operator,
        }
    }
}

// Position (stream_id, owner_addr) -> Position
pub const POSITIONS: Map<(StreamId, &Addr), Position> = Map::new("positions");
