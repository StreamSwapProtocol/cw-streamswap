use crate::state::Status;
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Binary, Coin, Decimal, Decimal256, Timestamp, Uint128};
use streamswap_factory::state::Params as FactoryParams;

#[cw_serde]
pub enum ExecuteMsg {
    /// Update stream and calculates distribution state.
    UpdateStream {},
    /// UpdateOperator updates the operator of the position.
    UpdateOperator {
        new_operator: Option<String>,
    },
    Subscribe {
        /// operator_target is the address of operator targets to execute on behalf of the user.
        operator_target: Option<String>,
        /// operator can subscribe/withdraw/update position.
        operator: Option<String>,
    },
    /// Withdraw unspent tokens in balance.
    Withdraw {
        cap: Option<Uint128>,
        /// operator_target is the address of operator targets to execute on behalf of the user.
        operator_target: Option<String>,
    },
    /// UpdatePosition updates the position of the user.
    /// syncs position index to the current state of the stream.
    UpdatePosition {
        /// operator_target is the address of operator targets to execute on behalf of the user.
        operator_target: Option<String>,
    },
    /// FinalizeStream clean ups the stream and sends income (earned tokens_in) to the
    /// Stream recipient. Returns error if called before the Stream end. Anyone can
    /// call this method.
    FinalizeStream {
        new_treasury: Option<String>,
    },
    /// ExitStream withdraws (by a user who subscribed to the stream) purchased
    /// tokens_out from the pool and remained tokens_in. Must be called after
    /// the stream ends.
    ExitStream {
        /// operator_target is the address of operator targets to execute on behalf of the user.
        operator_target: Option<String>,
        /// Salt is required for vested address generation
        salt: Option<Binary>,
    },
    //
    // Killswitch features
    //
    /// PauseStream pauses the stream. Only protocol admin and governance can pause the stream.
    PauseStream {},
    /// WithdrawPaused is used to withdraw unspent position funds during pause.
    WithdrawPaused {
        cap: Option<Uint128>,
        // operator_target is the address of operator targets to execute on behalf of the user.
        operator_target: Option<String>,
    },
    /// ExitCancelled returns the whole balance user put in the stream, both spent and unspent.
    ExitCancelled {
        /// Operator_target is the address of operator targets to execute on behalf of the user.
        operator_target: Option<String>,
    },
    ResumeStream {},
    CancelStream {},
    CancelStreamWithThreshold {},
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Returns current configuration.
    #[returns(FactoryParams)]
    Params {},
    /// Returns a stream's current state.
    #[returns(StreamResponse)]
    Stream {},
    /// Returns list of streams paginated by `start_after` and `limit`.
    // #[returns(StreamsResponse)]
    // ListStreams {
    //     start_after: Option<u64>,
    //     limit: Option<u32>,
    // },
    /// Returns current state of a position.
    #[returns(PositionResponse)]
    Position { owner: String },
    /// Returns list of positions paginated by `start_after` and `limit`.
    #[returns(PositionsResponse)]
    ListPositions {
        start_after: Option<String>,
        limit: Option<u32>,
    },
    /// Returns average price of a stream sale.
    #[returns(AveragePriceResponse)]
    AveragePrice {},
    /// Returns currently streaming price of a sale.
    #[returns(LatestStreamedPriceResponse)]
    LastStreamedPrice {},
    #[returns(Uint128)]
    Threshold {},
}

#[cw_serde]
pub struct ConfigResponse {
    /// Minimum seconds for a stream to last.
    pub min_stream_seconds: u64,
    /// Minimum seconds until the start time of a stream.
    pub min_seconds_until_start_time: u64,
    /// Denom accepted for subscription.
    pub accepted_in_denom: String,
    /// Denom used as fee for creating a stream.
    pub stream_creation_denom: String,
    /// Creation fee amount.
    pub stream_creation_fee: Uint128,
    /// This percentage represents the fee that will be collected from the investors.
    pub exit_fee_percent: Decimal,
    /// Address of the fee collector.
    pub fee_collector: String,
    /// Address of the protocol admin.
    pub protocol_admin: String,
}
#[cw_serde]
pub struct StreamResponse {
    /// Address of the treasury where the stream earnings will be sent.
    pub treasury: String,
    /// URL of the stream.
    pub url: Option<String>,
    /// Proportional distribution variable to calculate the distribution of in token_out to buyers.
    pub dist_index: Decimal256,
    /// Last updated time of stream.
    pub last_updated: Timestamp,

    pub out_asset: Coin,
    /// Total number of remaining out tokens at the time of update.
    pub out_remaining: Uint128,
    /// Denom of the `token_in`.
    pub in_denom: String,
    /// Total number of `token_in` on the buy side at latest state.
    pub in_supply: Uint128,
    /// Total number of `token_in` spent at latest state.
    pub spent_in: Uint128,
    /// Total number of shares minted.
    pub shares: Uint128,
    /// start time when the token emission starts. in nanos.
    pub start_time: Timestamp,
    /// end time when the token emission ends.
    pub end_time: Timestamp,
    /// Price at when latest distribution is triggered.
    pub current_streamed_price: Decimal,
    /// Status of the stream. Can be `Waiting`, `Active`, `Finalzed`, `Paused` or `Canceled` for kill switch.
    pub status: Status,
    /// second height when the stream was paused.
    pub pause_date: Option<Timestamp>,
    /// Address of the stream admin.
    pub stream_admin: String,
}

#[cw_serde]
pub struct StreamsResponse {
    pub streams: Vec<StreamResponse>,
}

#[cw_serde]
pub struct PositionResponse {
    /// Creator of the position.
    pub owner: String,
    /// Current amount of tokens in buy pool
    pub in_balance: Uint128,
    pub shares: Uint128,
    // Index is used to calculate the distribution a position has
    pub index: Decimal256,
    // Last_updated_time is the time when the position was last updated
    pub last_updated: Timestamp,
    // Total amount of `token_out` purchased in tokens at latest calculation
    pub purchased: Uint128,
    // Pending purchased accumulates purchases after decimal truncation
    pub pending_purchase: Decimal256,
    // Total amount of `token_in` spent tokens at latest calculation
    pub spent: Uint128,
    // Operator can update position
    pub operator: Option<Addr>,
}

#[cw_serde]
pub struct PositionsResponse {
    pub positions: Vec<PositionResponse>,
}

#[cw_serde]
pub struct AveragePriceResponse {
    pub average_price: Decimal,
}

#[cw_serde]
pub struct LatestStreamedPriceResponse {
    pub current_streamed_price: Decimal,
}

#[cw_serde]
pub enum SudoMsg {
    PauseStream {},
    CancelStream {},
    ResumeStream {},
}

#[cw_serde]
pub struct MigrateMsg {}
