use core::str;

use crate::helpers::{check_name_and_url, get_decimals};
use crate::killswitch::execute_cancel_stream_with_threshold;
use crate::{killswitch, ContractError};
use cosmwasm_std::{
    attr, coin, entry_point, to_json_binary, Addr, Attribute, BankMsg, Binary, CodeInfoResponse,
    Coin, CosmosMsg, Decimal, Decimal256, Deps, DepsMut, Env, MessageInfo, Order, Response,
    StdError, StdResult, Timestamp, Uint128, Uint256, WasmMsg,
};
use cw2::{ensure_from_older_version, set_contract_version};
use cw_storage_plus::Bound;
use cw_utils::{maybe_addr, must_pay};
use osmosis_std::types::cosmos::base;
use osmosis_std::types::osmosis::concentratedliquidity::v1beta1::MsgCreatePosition;
use osmosis_std::types::osmosis::poolmanager::v1beta1::PoolmanagerQuerier;
use streamswap_types::stream::ThresholdState;
use streamswap_types::stream::{
    AveragePriceResponse, ExecuteMsg, LatestStreamedPriceResponse, PositionResponse,
    PositionsResponse, QueryMsg, StreamResponse,
};

use crate::state::{FACTORY_PARAMS, POSITIONS, STREAM, VESTING};
use streamswap_types::factory::Params as FactoryParams;
use streamswap_types::factory::{CreateStreamMsg, MigrateMsg};
use streamswap_types::stream::{Position, Status, Stream};

// Version and contract info for migration
const CONTRACT_NAME: &str = "crates.io:streamswap-stream";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: CreateStreamMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    let params_query_msg = QueryMsg::Params {};
    let factory_params: FactoryParams = deps
        .querier
        .query_wasm_smart(info.sender.to_string(), &params_query_msg)?;
    // Factory parameters are collected at the time of stream creation
    // Any changes to factory parameters will not affect the stream
    FACTORY_PARAMS.save(deps.storage, &factory_params)?;

    let CreateStreamMsg {
        bootstraping_start_time,
        start_time,
        end_time,
        treasury,
        name,
        url,
        threshold,
        out_asset,
        in_denom,
        stream_admin,
        create_pool,
        vesting,
    } = msg;

    if start_time > end_time {
        return Err(ContractError::StreamInvalidEndTime {});
    }
    if env.block.time > start_time {
        return Err(ContractError::StreamInvalidStartTime {});
    }
    if in_denom == out_asset.denom {
        return Err(ContractError::SameDenomOnEachSide {});
    }
    if out_asset.amount.is_zero() {
        return Err(ContractError::ZeroOutSupply {});
    }
    let stream_admin = deps.api.addr_validate(&stream_admin)?;
    let treasury = deps.api.addr_validate(&treasury)?;

    check_name_and_url(&name, &url)?;

    let stream = Stream::new(
        env.block.time,
        name.clone(),
        treasury.clone(),
        stream_admin,
        url.clone(),
        out_asset.clone(),
        in_denom.clone(),
        bootstraping_start_time,
        start_time,
        end_time,
        start_time,
        create_pool,
        vesting,
    );
    STREAM.save(deps.storage, &stream)?;

    let threshold_state = ThresholdState::new();
    threshold_state.set_threshold_if_any(threshold, deps.storage)?;

    let attr = vec![
        attr("action", "create_stream"),
        attr("treasury", treasury),
        attr("name", name),
        attr("in_denom", in_denom),
        attr("out_denom", out_asset.denom),
        attr("out_supply", out_asset.amount.to_string()),
        attr("start_time", start_time.to_string()),
        attr("end_time", end_time.to_string()),
    ];
    Ok(Response::default().add_attributes(attr))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateOperator { new_operator } => {
            execute_update_operator(deps, env, info, new_operator)
        }
        ExecuteMsg::UpdatePosition { operator_target } => {
            execute_update_position(deps, env, info, operator_target)
        }
        ExecuteMsg::UpdateStream {} => execute_update_stream(deps, env),
        ExecuteMsg::Subscribe {
            operator_target,
            operator,
        } => {
            let stream = STREAM.load(deps.storage)?;
            execute_subscribe(deps, env, info, operator, operator_target, stream)
        }
        // let stream = STREAM.load(deps.storage)?;
        // if stream.start_time > env.block.time {
        //     Ok(execute_subscribe_pending(
        //         deps.branch(),
        //         env,
        //         info,
        //         operator,
        //         operator_target,
        //         stream,
        //     )?)
        // } else {
        //     Ok(execute_subscribe(
        //         deps,
        //         env,
        //         info,
        //         operator,
        //         operator_target,
        //         stream,
        //     )?)
        // }
        ExecuteMsg::Withdraw {
            cap,
            operator_target,
        } => {
            let stream = STREAM.load(deps.storage)?;
            execute_withdraw(deps, env, info, stream, cap, operator_target)
            // if stream.start_time > env.block.time {
            //     Ok(execute_withdraw_pending(
            //         deps.branch(),
            //         env,
            //         info,
            //         stream,
            //         cap,
            //         operator_target,
            //     )?)
            // } else {
            //     Ok(execute_withdraw(
            //         deps,
            //         env,
            //         info,
            //         stream,
            //         cap,
            //         operator_target,
            //     )?)
            // }
        }
        ExecuteMsg::FinalizeStream { new_treasury } => {
            execute_finalize_stream(deps, env, info, new_treasury)
        }
        ExecuteMsg::ExitStream {
            operator_target,
            salt,
        } => execute_exit_stream(deps, env, info, operator_target, salt),

        ExecuteMsg::CancelStream {} => killswitch::execute_cancel_stream(deps, env, info),
        ExecuteMsg::ExitCancelled { operator_target } => {
            killswitch::execute_exit_cancelled(deps, env, info, operator_target)
        }

        ExecuteMsg::CancelStreamWithThreshold {} => {
            execute_cancel_stream_with_threshold(deps, env, info)
        }
    }
}

/// Updates stream to calculate released distribution and spent amount
pub fn execute_update_stream(deps: DepsMut, env: Env) -> Result<Response, ContractError> {
    let mut stream = STREAM.load(deps.storage)?;
    stream.update(env.block.time);
    STREAM.save(deps.storage, &stream)?;

    let attrs = vec![
        attr("action", "update_stream"),
        // attr("new_distribution_amount", dist_amount),
        attr("dist_index", stream.dist_index.to_string()),
    ];
    let res = Response::new().add_attributes(attrs);
    Ok(res)
}

pub fn execute_update_position(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    operator_target: Option<String>,
) -> Result<Response, ContractError> {
    let operator_target =
        maybe_addr(deps.api, operator_target)?.unwrap_or_else(|| info.sender.clone());
    let mut position = POSITIONS.load(deps.storage, &operator_target)?;
    check_access(&info, &position.owner, &position.operator)?;

    let mut stream = STREAM.load(deps.storage)?;
    // check if stream is paused
    if stream.is_cancelled() {
        return Err(ContractError::StreamIsCancelled {});
    }

    // sync stream
    stream.update(env.block.time);
    STREAM.save(deps.storage, &stream)?;

    // updates position to latest distribution. Returns the amount of out tokens that has been purchased
    // and in tokens that has been spent.
    let (purchased, spent) = update_position(
        stream.dist_index,
        stream.shares,
        stream.last_updated,
        stream.in_supply,
        &mut position,
    )?;
    POSITIONS.save(deps.storage, &position.owner, &position)?;

    Ok(Response::new()
        .add_attribute("action", "update_position")
        .add_attribute("operator_target", operator_target)
        .add_attribute("purchased", purchased)
        .add_attribute("spent", spent))
}

// calculate the user purchase based on the positions index and the global index.
// returns purchased out amount and spent in amount
pub fn update_position(
    stream_dist_index: Decimal256,
    stream_shares: Uint128,
    stream_last_updated_time: Timestamp,
    stream_in_supply: Uint128,
    position: &mut Position,
) -> Result<(Uint128, Uint128), ContractError> {
    // index difference represents the amount of distribution that has been received since last update
    let index_diff = stream_dist_index.checked_sub(position.index)?;

    let mut spent = Uint128::zero();
    let mut purchased_uint128 = Uint128::zero();

    // if no shares available, means no distribution and no spent
    if !stream_shares.is_zero() {
        // purchased is index_diff * position.shares
        let purchased = Decimal256::from_ratio(position.shares, Uint256::one())
            .checked_mul(index_diff)?
            .checked_add(position.pending_purchase)?;
        // decimals is the amount of decimals that the out token has to be added to next distribution so that
        // the data do not get lost due to rounding
        let decimals = get_decimals(purchased)?;

        // calculates the remaining user balance using position.shares
        let in_remaining = stream_in_supply
            .checked_mul(position.shares)?
            .checked_div(stream_shares)?;

        // calculates the amount of spent tokens
        spent = position.in_balance.checked_sub(in_remaining)?;
        position.spent = position.spent.checked_add(spent)?;
        position.in_balance = in_remaining;
        position.pending_purchase = decimals;

        // floors the decimal points
        purchased_uint128 = (purchased * Uint256::one()).try_into()?;
        position.purchased = position.purchased.checked_add(purchased_uint128)?;
    }

    position.index = stream_dist_index;
    position.last_updated = stream_last_updated_time;

    Ok((purchased_uint128, spent))
}

pub fn execute_subscribe(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    operator: Option<String>,
    operator_target: Option<String>,
    mut stream: Stream,
) -> Result<Response, ContractError> {
    // check if stream is paused
    if stream.is_cancelled() {
        return Err(ContractError::StreamKillswitchActive {});
    }
    // Update stream status
    stream.update_status(env.block.time);

    if !(stream.is_active() || stream.is_bootstrapping()) {
        // TODO: create a new error for this
        return Err(ContractError::StreamNotStarted {});
    }
    // // On first subscibe change status to Active
    // if stream.status == Status::Waiting {
    //     stream.status = Status::Active
    // }

    let in_amount = must_pay(&info, &stream.in_denom)?;
    let new_shares;

    let operator = maybe_addr(deps.api, operator)?;
    let operator_target =
        maybe_addr(deps.api, operator_target)?.unwrap_or_else(|| info.sender.clone());
    let position = POSITIONS.may_load(deps.storage, &operator_target)?;
    match position {
        None => {
            // operator cannot create a position in behalf of anyone
            if operator_target != info.sender {
                return Err(ContractError::Unauthorized {});
            }
            // incoming tokens should not participate in prev distribution
            stream.update(env.block.time);
            new_shares = stream.compute_shares_amount(in_amount, false);
            // new positions do not update purchase as it has no effect on distribution
            let new_position = Position::new(
                info.sender,
                in_amount,
                new_shares,
                Some(stream.dist_index),
                operator,
                env.block.time,
            );
            POSITIONS.save(deps.storage, &operator_target, &new_position)?;
        }
        Some(mut position) => {
            check_access(&info, &position.owner, &position.operator)?;
            // incoming tokens should not participate in prev distribution
            stream.update(env.block.time);
            new_shares = stream.compute_shares_amount(in_amount, false);
            update_position(
                stream.dist_index,
                stream.shares,
                stream.last_updated,
                stream.in_supply,
                &mut position,
            )?;

            position.in_balance = position.in_balance.checked_add(in_amount)?;
            position.shares = position.shares.checked_add(new_shares)?;
            POSITIONS.save(deps.storage, &operator_target, &position)?;
        }
    }

    // increase in supply and shares
    stream.in_supply = stream.in_supply.checked_add(in_amount)?;
    stream.shares = stream.shares.checked_add(new_shares)?;
    STREAM.save(deps.storage, &stream)?;

    let res = Response::new()
        .add_attribute("action", "subscribe")
        .add_attribute("owner", operator_target)
        .add_attribute("in_supply", stream.in_supply)
        .add_attribute("in_amount", in_amount);

    Ok(res)
}

// pub fn execute_subscribe_pending(
//     deps: DepsMut,
//     env: Env,
//     info: MessageInfo,
//     operator: Option<String>,
//     operator_target: Option<String>,
//     mut stream: Stream,
// ) -> Result<Response, ContractError> {
//     // check if stream is paused
//     if stream.is_killswitch_active() {
//         return Err(ContractError::StreamKillswitchActive {});
//     }
//     let in_amount = must_pay(&info, &stream.in_denom)?;
//     let new_shares = stream.compute_shares_amount(in_amount, false);

//     let operator = maybe_addr(deps.api, operator)?;
//     let operator_target =
//         maybe_addr(deps.api, operator_target)?.unwrap_or_else(|| info.sender.clone());
//     let position = POSITIONS.may_load(deps.storage, &operator_target)?;
//     match position {
//         None => {
//             // operator cannot create a position in behalf of anyone
//             if operator_target != info.sender {
//                 return Err(ContractError::Unauthorized {});
//             }
//             let new_position = Position::new(
//                 info.sender,
//                 in_amount,
//                 new_shares,
//                 Some(stream.dist_index),
//                 operator,
//                 env.block.time,
//             );
//             POSITIONS.save(deps.storage, &operator_target, &new_position)?;
//         }
//         Some(mut position) => {
//             check_access(&info, &position.owner, &position.operator)?;
//             // if subscibed already, we wont update its position but just increase its in_balance and shares
//             position.in_balance = position.in_balance.checked_add(in_amount)?;
//             position.shares = position.shares.checked_add(new_shares)?;
//             POSITIONS.save(deps.storage, &operator_target, &position)?;
//         }
//     }
//     stream.in_supply = stream.in_supply.checked_add(in_amount)?;
//     stream.shares = stream.shares.checked_add(new_shares)?;
//     STREAM.save(deps.storage, &stream)?;

//     Ok(Response::new()
//         .add_attribute("action", "subscribe_pending")
//         .add_attribute("owner", operator_target)
//         .add_attribute("in_supply", stream.in_supply)
//         .add_attribute("in_amount", in_amount))
// }

pub fn execute_update_operator(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    operator: Option<String>,
) -> Result<Response, ContractError> {
    let mut position = POSITIONS.load(deps.storage, &info.sender)?;

    let operator = maybe_addr(deps.api, operator)?;
    position.operator = operator.clone();

    POSITIONS.save(deps.storage, &info.sender, &position)?;

    Ok(Response::new()
        .add_attribute("action", "update_operator")
        .add_attribute("owner", info.sender)
        .add_attribute("operator", operator.unwrap_or_else(|| Addr::unchecked(""))))
}

pub fn execute_withdraw(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    mut stream: Stream,
    cap: Option<Uint128>,
    operator_target: Option<String>,
) -> Result<Response, ContractError> {
    // // check if stream is paused
    // if stream.is_killswitch_active() {
    //     return Err(ContractError::StreamKillswitchActive {});
    // }
    stream.update_status(env.block.time);
    if !(stream.is_active() || stream.is_bootstrapping()) {
        // TODO: create a new error for this
        return Err(ContractError::StreamNotStarted {});
    }

    let operator_target =
        maybe_addr(deps.api, operator_target)?.unwrap_or_else(|| info.sender.clone());
    let mut position = POSITIONS.load(deps.storage, &operator_target)?;
    check_access(&info, &position.owner, &position.operator)?;

    //update_stream(env.block.time, &mut stream)?;
    stream.update(env.block.time);
    update_position(
        stream.dist_index,
        stream.shares,
        stream.last_updated,
        stream.in_supply,
        &mut position,
    )?;

    let withdraw_amount = cap.unwrap_or(position.in_balance);
    // if amount to withdraw more then deduced buy balance throw error
    if withdraw_amount > position.in_balance {
        return Err(ContractError::WithdrawAmountExceedsBalance(withdraw_amount));
    }

    if withdraw_amount.is_zero() {
        return Err(ContractError::InvalidWithdrawAmount {});
    }

    // decrease in supply and shares
    let shares_amount = if withdraw_amount == position.in_balance {
        position.shares
    } else {
        stream.compute_shares_amount(withdraw_amount, true)
    };

    stream.in_supply = stream.in_supply.checked_sub(withdraw_amount)?;
    stream.shares = stream.shares.checked_sub(shares_amount)?;
    position.in_balance = position.in_balance.checked_sub(withdraw_amount)?;
    position.shares = position.shares.checked_sub(shares_amount)?;

    STREAM.save(deps.storage, &stream)?;
    POSITIONS.save(deps.storage, &position.owner, &position)?;

    let attributes = vec![
        attr("action", "withdraw"),
        attr("operator_target", operator_target.clone()),
        attr("withdraw_amount", withdraw_amount),
    ];

    // send funds to withdraw address or to the sender
    let res = Response::new()
        .add_message(CosmosMsg::Bank(BankMsg::Send {
            to_address: operator_target.to_string(),
            amount: vec![Coin {
                denom: stream.in_denom,
                amount: withdraw_amount,
            }],
        }))
        .add_attributes(attributes);

    Ok(res)
}

// pub fn execute_withdraw_pending(
//     deps: DepsMut,
//     _env: Env,
//     info: MessageInfo,
//     mut stream: Stream,
//     cap: Option<Uint128>,
//     operator_target: Option<String>,
// ) -> Result<Response, ContractError> {
//     // check if stream is paused
//     let operator_target =
//         maybe_addr(deps.api, operator_target)?.unwrap_or_else(|| info.sender.clone());
//     let mut position = POSITIONS.load(deps.storage, &operator_target)?;
//     check_access(&info, &position.owner, &position.operator)?;

//     let withdraw_amount = cap.unwrap_or(position.in_balance);
//     // if amount to withdraw more then deduced buy balance throw error
//     if withdraw_amount > position.in_balance {
//         return Err(ContractError::WithdrawAmountExceedsBalance(withdraw_amount));
//     }

//     if withdraw_amount.is_zero() {
//         return Err(ContractError::InvalidWithdrawAmount {});
//     }

//     // decrease in supply and shares
//     let shares_amount = if withdraw_amount == position.in_balance {
//         position.shares
//     } else {
//         stream.compute_shares_amount(withdraw_amount, true)
//     };

//     stream.in_supply = stream.in_supply.checked_sub(withdraw_amount)?;
//     stream.shares = stream.shares.checked_sub(shares_amount)?;
//     position.in_balance = position.in_balance.checked_sub(withdraw_amount)?;
//     position.shares = position.shares.checked_sub(shares_amount)?;

//     STREAM.save(deps.storage, &stream)?;
//     POSITIONS.save(deps.storage, &position.owner, &position)?;

//     let attributes = vec![
//         attr("action", "withdraw_pending"),
//         attr("operator_target", operator_target.clone()),
//         attr("withdraw_amount", withdraw_amount),
//     ];

//     // send funds to withdraw address or to the sender
//     let res = Response::new()
//         .add_message(CosmosMsg::Bank(BankMsg::Send {
//             to_address: operator_target.to_string(),
//             amount: vec![Coin {
//                 denom: stream.in_denom,
//                 amount: withdraw_amount,
//             }],
//         }))
//         .add_attributes(attributes);

//     Ok(res)
// }

pub fn execute_finalize_stream(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    new_treasury: Option<String>,
) -> Result<Response, ContractError> {
    let mut stream = STREAM.load(deps.storage)?;
    // check if the stream is already finalized
    if stream.is_finalized() {
        return Err(ContractError::StreamAlreadyFinalized {});
    }
    // check if killswitch is active
    if stream.is_cancelled() {
        // TODO: create a new error for this
        return Err(ContractError::StreamKillswitchActive {});
    }
    if stream.treasury != info.sender {
        return Err(ContractError::Unauthorized {});
    }
    stream.update_status(env.block.time);
    if !stream.is_ended() {
        return Err(ContractError::StreamNotEnded {});
    }
    stream.update(env.block.time);

    stream.status.status = Status::Finalized;

    // If threshold is set and not reached, finalize will fail
    // Creator should execute cancel_stream_with_threshold to cancel the stream
    // Only returns error if threshold is set and not reached
    let thresholds_state = ThresholdState::new();
    thresholds_state.error_if_not_reached(deps.storage, &stream)?;

    STREAM.save(deps.storage, &stream)?;

    let factory_params = FACTORY_PARAMS.load(deps.storage)?;
    let treasury = maybe_addr(deps.api, new_treasury)?.unwrap_or_else(|| stream.treasury.clone());

    //Stream's swap fee collected at fixed rate from accumulated spent_in of positions(ie stream.spent_in)
    let swap_fee = Decimal::from_ratio(stream.spent_in, Uint128::one())
        .checked_mul(factory_params.exit_fee_percent)?
        * Uint128::one();

    let creator_revenue = stream.spent_in.checked_sub(swap_fee)?;

    let mut messages = vec![];
    //Creator's revenue claimed at finalize
    let revenue_msg = CosmosMsg::Bank(BankMsg::Send {
        to_address: treasury.to_string(),
        amount: vec![Coin {
            denom: stream.in_denom.clone(),
            amount: creator_revenue,
        }],
    });
    messages.push(revenue_msg);
    let swap_fee_msg = CosmosMsg::Bank(BankMsg::Send {
        to_address: factory_params.fee_collector.to_string(),
        amount: vec![Coin {
            denom: stream.in_denom.clone(),
            amount: swap_fee,
        }],
    });
    messages.push(swap_fee_msg);

    // if no spent, remove all messages to prevent failure
    if stream.spent_in == Uint128::zero() {
        messages = vec![]
    }

    // In case the stream is ended without any shares in it. We need to refund the remaining
    // out tokens although that is unlikely to happen.
    if stream.out_remaining > Uint128::zero() {
        let remaining_out = stream.out_remaining;
        let remaining_msg = CosmosMsg::Bank(BankMsg::Send {
            to_address: treasury.to_string(),
            amount: vec![Coin {
                denom: stream.out_asset.denom.clone(),
                amount: remaining_out,
            }],
        });
        messages.push(remaining_msg);
    }
    if let Some(pool) = stream.create_pool {
        messages.push(pool.msg_create_pool.into());

        // amount of in tokens allocated for clp
        let in_clp = (pool.out_amount_clp / stream.out_asset.amount) * stream.spent_in;
        let current_num_of_pools = PoolmanagerQuerier::new(&deps.querier)
            .num_pools()?
            .num_pools;
        let pool_id = current_num_of_pools + 1;

        let create_initial_position_msg = MsgCreatePosition {
            pool_id,
            sender: treasury.to_string(),
            lower_tick: 0,
            upper_tick: i64::MAX,
            tokens_provided: vec![
                base::v1beta1::Coin {
                    denom: stream.in_denom,
                    amount: in_clp.to_string(),
                },
                base::v1beta1::Coin {
                    denom: stream.out_asset.denom,
                    amount: pool.out_amount_clp.to_string(),
                },
            ],
            token_min_amount0: "0".to_string(),
            token_min_amount1: "0".to_string(),
        };
        messages.push(create_initial_position_msg.into());
    }

    Ok(Response::new().add_messages(messages).add_attributes(vec![
        attr("action", "finalize_stream"),
        attr("treasury", treasury.as_str()),
        attr("fee_collector", factory_params.fee_collector.to_string()),
        attr("creators_revenue", creator_revenue),
        attr("refunded_out_remaining", stream.out_remaining.to_string()),
        attr(
            "total_sold",
            stream
                .out_asset
                .amount
                .checked_sub(stream.out_remaining)?
                .to_string(),
        ),
        attr("swap_fee", swap_fee),
        attr(
            "creation_fee_amount",
            factory_params.stream_creation_fee.amount.to_string(),
        ),
    ]))
}

pub fn execute_exit_stream(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    operator_target: Option<String>,
    salt: Option<Binary>,
) -> Result<Response, ContractError> {
    let mut stream = STREAM.load(deps.storage)?;
    let factory_params = FACTORY_PARAMS.load(deps.storage)?;
    // check if stream is paused
    if stream.is_cancelled() {
        return Err(ContractError::StreamKillswitchActive {});
    }
    stream.update_status(env.block.time);

    if !stream.is_ended() {
        return Err(ContractError::StreamNotEnded {});
    }
    stream.update(env.block.time);

    let threshold_state = ThresholdState::new();

    threshold_state.error_if_not_reached(deps.storage, &stream)?;

    let operator_target =
        maybe_addr(deps.api, operator_target)?.unwrap_or_else(|| info.sender.clone());
    let mut position = POSITIONS.load(deps.storage, &operator_target)?;
    check_access(&info, &position.owner, &position.operator)?;

    // update position before exit
    update_position(
        stream.dist_index,
        stream.shares,
        stream.last_updated,
        stream.in_supply,
        &mut position,
    )?;
    stream.shares = stream.shares.checked_sub(position.shares)?;

    STREAM.save(deps.storage, &stream)?;
    POSITIONS.remove(deps.storage, &position.owner);

    // Swap fee = fixed_rate*position.spent_in this calculation is only for execution reply attributes
    let swap_fee = Decimal::from_ratio(position.spent, Uint128::one())
        .checked_mul(factory_params.exit_fee_percent)?
        * Uint128::one();

    let mut msgs: Vec<CosmosMsg> = vec![];
    let mut attrs: Vec<Attribute> = vec![];

    // if vesting is set, instantiate a vested release contract for user and send
    // the out tokens to the contract
    if let Some(mut vesting) = stream.vesting {
        let salt = salt.ok_or(ContractError::InvalidSalt {})?;

        // prepare vesting msg
        vesting.start_time = Some(stream.status.end_time);
        // TODO: check if we want an owner?
        vesting.owner = None;
        vesting.recipient = operator_target.to_string();
        vesting.total = position.purchased;

        // prepare instantiate msg msg
        let CodeInfoResponse { checksum, .. } = deps
            .querier
            .query_wasm_code_info(factory_params.vesting_code_id)?;
        let creator = deps.api.addr_canonicalize(env.contract.address.as_str())?;

        // Calculate the address of the new contract
        let address = deps.api.addr_humanize(&cosmwasm_std::instantiate2_address(
            checksum.as_ref(),
            &creator,
            &salt,
        )?)?;

        VESTING.save(deps.storage, operator_target.clone(), &address)?;

        let vesting_instantiate_msg = WasmMsg::Instantiate2 {
            admin: None,
            code_id: factory_params.vesting_code_id,
            label: format!(
                "streamswap: Stream Addr {} Released to {}",
                env.contract.address, operator_target
            ),
            msg: to_json_binary(&vesting)?,
            funds: vec![coin(position.purchased.u128(), stream.out_asset.denom)],
            salt,
        };

        msgs.push(vesting_instantiate_msg.into());
        attrs.push(attr("vesting_address", address));
    } else {
        let send_msg = CosmosMsg::Bank(BankMsg::Send {
            to_address: operator_target.to_string(),
            amount: vec![Coin {
                denom: stream.out_asset.denom.to_string(),
                amount: position.purchased,
            }],
        });
        msgs.push(send_msg);
    }
    // if there is any unspent in balance, send it back to the user
    if !position.in_balance.is_zero() {
        let unspent = position.in_balance;
        let unspent_msg = CosmosMsg::Bank(BankMsg::Send {
            to_address: operator_target.to_string(),
            amount: vec![Coin {
                denom: stream.in_denom,
                amount: unspent,
            }],
        });
        msgs.push(unspent_msg);
    }

    attrs.extend(vec![
        attr("action", "exit_stream"),
        attr("spent", position.spent.checked_sub(swap_fee)?),
        attr("purchased", position.purchased),
        attr("swap_fee_paid", swap_fee),
    ]);

    Ok(Response::new().add_messages(msgs).add_attributes(attrs))
}

fn check_access(
    info: &MessageInfo,
    position_owner: &Addr,
    position_operator: &Option<Addr>,
) -> Result<(), ContractError> {
    if position_owner.as_ref() != info.sender
        && position_operator
            .as_ref()
            .map_or(true, |o| o != info.sender)
    {
        return Err(ContractError::Unauthorized {});
    }
    Ok(())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    ensure_from_older_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Params {} => to_json_binary(&query_params(deps)?),
        QueryMsg::Stream {} => to_json_binary(&query_stream(deps, env)?),
        QueryMsg::Position { owner } => to_json_binary(&query_position(deps, env, owner)?),
        // QueryMsg::ListStreams { start_after, limit } => {
        //     to_json_binary(&list_streams(deps, start_after, limit)?)
        // }
        QueryMsg::ListPositions { start_after, limit } => {
            to_json_binary(&list_positions(deps, start_after, limit)?)
        }
        QueryMsg::AveragePrice {} => to_json_binary(&query_average_price(deps, env)?),
        QueryMsg::LastStreamedPrice {} => to_json_binary(&query_last_streamed_price(deps, env)?),
        QueryMsg::Threshold {} => to_json_binary(&query_threshold_state(deps, env)?),
    }
}
pub fn query_params(deps: Deps) -> StdResult<FactoryParams> {
    let factory_params = FACTORY_PARAMS.load(deps.storage)?;
    Ok(factory_params)
}

pub fn query_stream(deps: Deps, _env: Env) -> StdResult<StreamResponse> {
    let stream = STREAM.load(deps.storage)?;
    let stream = StreamResponse {
        treasury: stream.treasury.to_string(),
        in_denom: stream.in_denom,
        out_asset: stream.out_asset,
        start_time: stream.status.start_time,
        end_time: stream.status.end_time,
        last_updated: stream.last_updated,
        spent_in: stream.spent_in,
        dist_index: stream.dist_index,
        out_remaining: stream.out_remaining,
        in_supply: stream.in_supply,
        shares: stream.shares,
        status: stream.status.status,
        url: stream.url,
        current_streamed_price: stream.current_streamed_price,
        stream_admin: stream.stream_admin.into_string(),
    };
    Ok(stream)
}

// // settings for pagination
// const MAX_LIMIT: u32 = 30;
// const DEFAULT_LIMIT: u32 = 10;

// pub fn list_streams(
//     deps: Deps,
//     start_after: Option<u64>,
//     limit: Option<u32>,
// ) -> StdResult<StreamsResponse> {
//     let start = start_after.map(Bound::exclusive);
//     let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
//     let streams: StdResult<Vec<StreamResponse>> = STREAMS
//         .range(deps.storage, start, None, Order::Ascending)
//         .take(limit)
//         .map(|item| {
//             let (stream_id, stream) = item?;
//             let stream = StreamResponse {
//                 id: stream_id,
//                 treasury: stream.treasury.to_string(),
//                 in_denom: stream.in_denom,
//                 out_asset: stream.out_asset,
//                 start_time: stream.start_time,
//                 end_time: stream.end_time,
//                 last_updated: stream.last_updated,
//                 spent_in: stream.spent_in,
//                 dist_index: stream.dist_index,
//                 out_remaining: stream.out_remaining,
//                 in_supply: stream.in_supply,
//                 shares: stream.shares,
//                 status: stream.status,
//                 pause_date: stream.pause_date,
//                 url: stream.url,
//                 current_streamed_price: stream.current_streamed_price,
//                 stream_admin: stream.stream_admin.into_string(),
//             };
//             Ok(stream)
//         })
//         .collect();
//     let streams = streams?;
//     Ok(StreamsResponse { streams })
// }

pub fn query_position(deps: Deps, _env: Env, owner: String) -> StdResult<PositionResponse> {
    let owner = deps.api.addr_validate(&owner)?;
    let position = POSITIONS.load(deps.storage, &owner)?;
    let res = PositionResponse {
        owner: owner.to_string(),
        in_balance: position.in_balance,
        purchased: position.purchased,
        index: position.index,
        spent: position.spent,
        shares: position.shares,
        operator: position.operator,
        last_updated: position.last_updated,
        pending_purchase: position.pending_purchase,
    };
    Ok(res)
}

pub fn list_positions(
    deps: Deps,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<PositionsResponse> {
    const MAX_LIMIT: u32 = 30;
    let start_addr = maybe_addr(deps.api, start_after)?;
    let start = start_addr.as_ref().map(Bound::exclusive);
    let limit = limit.unwrap_or(MAX_LIMIT).min(MAX_LIMIT) as usize;
    let positions: StdResult<Vec<PositionResponse>> = POSITIONS
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|item| {
            let (owner, position) = item?;
            let position = PositionResponse {
                owner: owner.to_string(),
                in_balance: position.in_balance,
                purchased: position.purchased,
                index: position.index,
                spent: position.spent,
                shares: position.shares,
                operator: position.operator,
                last_updated: position.last_updated,
                pending_purchase: position.pending_purchase,
            };
            Ok(position)
        })
        .collect();
    let positions = positions?;
    Ok(PositionsResponse { positions })
}

pub fn query_average_price(deps: Deps, _env: Env) -> StdResult<AveragePriceResponse> {
    let stream = STREAM.load(deps.storage)?;
    let total_purchased = stream.out_asset.amount - stream.out_remaining;
    let average_price = Decimal::from_ratio(stream.spent_in, total_purchased);
    Ok(AveragePriceResponse { average_price })
}

pub fn query_last_streamed_price(deps: Deps, _env: Env) -> StdResult<LatestStreamedPriceResponse> {
    let stream = STREAM.load(deps.storage)?;
    Ok(LatestStreamedPriceResponse {
        current_streamed_price: stream.current_streamed_price,
    })
}

pub fn query_threshold_state(deps: Deps, _env: Env) -> Result<Option<Uint128>, StdError> {
    let threshold_state = ThresholdState::new();
    let threshold = threshold_state.get_threshold(deps.storage)?;
    Ok(threshold)
}
