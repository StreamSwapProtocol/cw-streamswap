use core::str;
use std::env;

use crate::helpers::{check_name_and_url, get_decimals, validate_stream_times};
use crate::killswitch::execute_cancel_stream_with_threshold;
use crate::stream::{compute_shares_amount, sync_stream_status, update_stream};
use crate::{killswitch, ContractError};
use cosmwasm_std::{
    attr, coin, entry_point, to_json_binary, Attribute, BankMsg, Binary, CodeInfoResponse, Coin,
    CosmosMsg, Decimal256, Deps, DepsMut, Env, MessageInfo, Order, Response, StdError, StdResult,
    Timestamp, Uint128, Uint256, WasmMsg,
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
use streamswap_utils::payment_checker::check_payment;
use streamswap_utils::to_uint256;

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
        salt: _,
    } = msg;
    // Check if out asset is provided
    // TODO: This might be unnecessary as we are checking this at factory level
    check_payment(&info.funds, &[out_asset.clone()])?;

    validate_stream_times(
        env.block.time,
        bootstraping_start_time,
        start_time,
        end_time,
        &factory_params,
    )?;

    if in_denom == out_asset.denom {
        return Err(ContractError::SameDenomOnEachSide {});
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
        ExecuteMsg::UpdatePosition {} => execute_update_position(deps, env, info),
        ExecuteMsg::UpdateStream {} => execute_update_stream(deps, env),
        ExecuteMsg::Subscribe {} => {
            let stream = STREAM.load(deps.storage)?;
            execute_subscribe(deps, env, info, stream)
        }
        ExecuteMsg::Withdraw { cap } => {
            let stream = STREAM.load(deps.storage)?;
            execute_withdraw(deps, env, info, stream, cap)
        }
        ExecuteMsg::FinalizeStream { new_treasury } => {
            execute_finalize_stream(deps, env, info, new_treasury)
        }
        ExecuteMsg::ExitStream { salt } => execute_exit_stream(deps, env, info, salt),
        ExecuteMsg::CancelStream {} => killswitch::execute_cancel_stream(deps, env, info),
        ExecuteMsg::ExitCancelled {} => killswitch::execute_exit_cancelled(deps, env, info),
        ExecuteMsg::CancelStreamWithThreshold {} => {
            execute_cancel_stream_with_threshold(deps, env, info)
        }
        ExecuteMsg::StreamAdminCancel {} => {
            killswitch::execute_stream_admin_cancel(deps, env, info)
        }
    }
}

/// Updates stream to calculate released distribution and spent amount
pub fn execute_update_stream(deps: DepsMut, env: Env) -> Result<Response, ContractError> {
    let mut stream = STREAM.load(deps.storage)?;
    update_stream(&mut stream, env.block.time);
    sync_stream_status(&mut stream, env.block.time);
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
) -> Result<Response, ContractError> {
    let mut position = POSITIONS.load(deps.storage, &info.sender)?;

    let mut stream = STREAM.load(deps.storage)?;
    // check if stream is cancelled
    if stream.is_cancelled() {
        return Err(ContractError::StreamIsCancelled {});
    }

    // sync stream
    update_stream(&mut stream, env.block.time);
    sync_stream_status(&mut stream, env.block.time);
    STREAM.save(deps.storage, &stream)?;

    // updates position to latest distribution. Returns the amount of out tokens that has been purchased
    // and in tokens that has been spent.
    let (purchased, spent) = update_position(
        stream.dist_index,
        stream.shares,
        stream.status_info.last_updated,
        stream.in_supply,
        &mut position,
    )?;
    POSITIONS.save(deps.storage, &position.owner, &position)?;

    Ok(Response::new()
        .add_attribute("action", "update_position")
        .add_attribute("purchased", purchased)
        .add_attribute("spent", spent))
}

// calculate the user purchase based on the positions index and the global index.
// returns purchased out amount and spent in amount
pub fn update_position(
    stream_dist_index: Decimal256,
    stream_shares: Uint256,
    stream_last_updated_time: Timestamp,
    stream_in_supply: Uint256,
    position: &mut Position,
) -> Result<(Uint256, Uint256), ContractError> {
    // index difference represents the amount of distribution that has been received since last update
    let index_diff = stream_dist_index.checked_sub(position.index)?;

    let mut spent = Uint256::zero();
    let mut uint256_purchased = Uint256::zero();

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
        uint256_purchased = purchased * Uint256::one();
        position.purchased = position.purchased.checked_add(uint256_purchased)?;
    }

    position.index = stream_dist_index;
    position.last_updated = stream_last_updated_time;

    Ok((uint256_purchased, spent))
}

pub fn execute_subscribe(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    mut stream: Stream,
) -> Result<Response, ContractError> {
    // Check if stream is cancelled
    if stream.is_cancelled() {
        return Err(ContractError::StreamKillswitchActive {});
    }
    // Update stream status
    sync_stream_status(&mut stream, env.block.time);

    if !(stream.is_active() || stream.is_bootstrapping()) {
        return Err(ContractError::StreamNotStarted {});
    }

    let in_amount = must_pay(&info, &stream.in_denom)?;
    let uint256_in_amount = Uint256::from(in_amount.u128());
    let new_shares;

    let position = POSITIONS.may_load(deps.storage, &info.sender)?;
    match position {
        None => {
            // incoming tokens should not participate in prev distribution
            update_stream(&mut stream, env.block.time);
            new_shares = compute_shares_amount(&stream, uint256_in_amount, false);
            // new positions do not update purchase as it has no effect on distribution
            let new_position = Position::new(
                info.sender.clone(),
                uint256_in_amount,
                new_shares,
                Some(stream.dist_index),
                env.block.time,
            );
            POSITIONS.save(deps.storage, &info.sender, &new_position)?;
        }
        Some(mut position) => {
            if position.owner != info.sender {
                return Err(ContractError::Unauthorized {});
            }
            // incoming tokens should not participate in prev distribution
            update_stream(&mut stream, env.block.time);
            new_shares = compute_shares_amount(&stream, uint256_in_amount, false);
            update_position(
                stream.dist_index,
                stream.shares,
                stream.status_info.last_updated,
                stream.in_supply,
                &mut position,
            )?;

            position.in_balance = position.in_balance.checked_add(uint256_in_amount)?;
            position.shares = position.shares.checked_add(new_shares)?;
            POSITIONS.save(deps.storage, &info.sender, &position)?;
        }
    }

    // increase in supply and shares
    stream.in_supply = stream.in_supply.checked_add(uint256_in_amount)?;
    stream.shares = stream.shares.checked_add(new_shares)?;
    STREAM.save(deps.storage, &stream)?;

    let res = Response::new()
        .add_attribute("action", "subscribe")
        .add_attribute("in_supply", stream.in_supply)
        .add_attribute("in_amount", in_amount);

    Ok(res)
}

pub fn execute_withdraw(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    mut stream: Stream,
    cap: Option<Uint256>,
) -> Result<Response, ContractError> {
    sync_stream_status(&mut stream, env.block.time);
    if !(stream.is_active() || stream.is_bootstrapping()) {
        // TODO: create a new error for this
        return Err(ContractError::StreamNotStarted {});
    }

    let mut position = POSITIONS.load(deps.storage, &info.sender)?;

    update_stream(&mut stream, env.block.time);
    update_position(
        stream.dist_index,
        stream.shares,
        stream.status_info.last_updated,
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
        compute_shares_amount(&stream, withdraw_amount, true)
    };

    stream.in_supply = stream.in_supply.checked_sub(withdraw_amount)?;
    stream.shares = stream.shares.checked_sub(shares_amount)?;
    position.in_balance = position.in_balance.checked_sub(withdraw_amount)?;
    position.shares = position.shares.checked_sub(shares_amount)?;

    STREAM.save(deps.storage, &stream)?;
    POSITIONS.save(deps.storage, &position.owner, &position)?;

    let attributes = vec![
        attr("action", "withdraw"),
        attr("withdraw_amount", withdraw_amount),
    ];

    let uint128_withdraw_amount = Uint128::try_from(withdraw_amount)?;
    // send funds to withdraw address or to the sender
    let res = Response::new()
        .add_message(CosmosMsg::Bank(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: vec![Coin {
                denom: stream.in_denom,
                amount: uint128_withdraw_amount,
            }],
        }))
        .add_attributes(attributes);

    Ok(res)
}
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
    sync_stream_status(&mut stream, env.block.time);

    if !stream.is_ended() {
        return Err(ContractError::StreamNotEnded {});
    }
    update_stream(&mut stream, env.block.time);

    stream.status_info.status = Status::Finalized;

    // If threshold is set and not reached, finalize will fail
    // Creator should execute cancel_stream_with_threshold to cancel the stream
    // Only returns error if threshold is set and not reached
    let thresholds_state = ThresholdState::new();
    thresholds_state.error_if_not_reached(deps.storage, &stream)?;

    STREAM.save(deps.storage, &stream)?;

    let factory_params = FACTORY_PARAMS.load(deps.storage)?;
    let treasury = maybe_addr(deps.api, new_treasury)?.unwrap_or_else(|| stream.treasury.clone());

    //Stream's swap fee collected at fixed rate from accumulated spent_in of positions(ie stream.spent_in)
    let swap_fee = Decimal256::from_ratio(stream.spent_in, Uint128::one())
        .checked_mul(factory_params.exit_fee_percent)?
        * Uint256::one();

    let creator_revenue = stream.spent_in.checked_sub(swap_fee)?;

    let mut messages = vec![];
    let uint128_creator_revenue = Uint128::try_from(creator_revenue)?;
    //Creator's revenue claimed at finalize
    let revenue_msg = CosmosMsg::Bank(BankMsg::Send {
        to_address: treasury.to_string(),
        amount: vec![Coin {
            denom: stream.in_denom.clone(),
            amount: uint128_creator_revenue,
        }],
    });
    messages.push(revenue_msg);
    let uint128_swap_fee = Uint128::try_from(swap_fee)?;
    let swap_fee_msg = CosmosMsg::Bank(BankMsg::Send {
        to_address: factory_params.fee_collector.to_string(),
        amount: vec![Coin {
            denom: stream.in_denom.clone(),
            amount: uint128_swap_fee,
        }],
    });
    messages.push(swap_fee_msg);

    // if no spent, remove all messages to prevent failure
    if stream.spent_in == Uint256::zero() {
        messages = vec![]
    }

    // In case the stream is ended without any shares in it. We need to refund the remaining
    // out tokens although that is unlikely to happen.
    if stream.out_remaining > Uint256::zero() {
        let remaining_out = stream.out_remaining;
        let uint128_remaining_out = Uint128::try_from(remaining_out)?;
        let remaining_msg = CosmosMsg::Bank(BankMsg::Send {
            to_address: treasury.to_string(),
            amount: vec![Coin {
                denom: stream.out_asset.denom.clone(),
                amount: uint128_remaining_out,
            }],
        });
        messages.push(remaining_msg);
    }
    if let Some(pool) = stream.create_pool {
        messages.push(pool.msg_create_pool.into());

        // amount of in tokens allocated for clp
        let in_clp = (pool.out_amount_clp / to_uint256(stream.out_asset.amount)) * stream.spent_in;
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
            to_uint256(stream.out_asset.amount)
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
    salt: Option<Binary>,
) -> Result<Response, ContractError> {
    let mut stream = STREAM.load(deps.storage)?;
    let factory_params = FACTORY_PARAMS.load(deps.storage)?;
    // check if stream is paused
    if stream.is_cancelled() {
        return Err(ContractError::StreamKillswitchActive {});
    }
    sync_stream_status(&mut stream, env.block.time);

    if !(stream.is_ended() || stream.is_finalized()) {
        return Err(ContractError::StreamNotEnded {});
    }
    update_stream(&mut stream, env.block.time);

    let threshold_state = ThresholdState::new();

    threshold_state.error_if_not_reached(deps.storage, &stream)?;

    let mut position = POSITIONS.load(deps.storage, &info.sender)?;
    // TODO: add test case for this
    if position.exit_date != Timestamp::from_seconds(0) {
        return Err(ContractError::SubscriberAlreadyExited {});
    }

    // update position before exit
    update_position(
        stream.dist_index,
        stream.shares,
        stream.status_info.last_updated,
        stream.in_supply,
        &mut position,
    )?;
    stream.shares = stream.shares.checked_sub(position.shares)?;

    STREAM.save(deps.storage, &stream)?;
    // update position exit date
    position.exit_date = env.block.time;
    POSITIONS.save(deps.storage, &position.owner, &position)?;

    // Swap fee = fixed_rate*position.spent_in this calculation is only for execution reply attributes
    let swap_fee = Decimal256::from_ratio(position.spent, Uint128::one())
        .checked_mul(factory_params.exit_fee_percent)?
        * Uint256::one();

    let mut msgs: Vec<CosmosMsg> = vec![];
    let mut attrs: Vec<Attribute> = vec![];

    // if vesting is set, instantiate a vested release contract for user and send
    // the out tokens to the contract
    let uint128_purchased = Uint128::try_from(position.purchased)?;
    if let Some(mut vesting) = stream.vesting {
        let salt = salt.ok_or(ContractError::InvalidSalt {})?;

        // prepare vesting msg
        vesting.start_time = Some(stream.status_info.end_time);
        vesting.owner = None;
        vesting.recipient = info.sender.to_string();
        vesting.total = uint128_purchased;

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

        VESTING.save(deps.storage, info.sender.clone(), &address)?;

        let vesting_instantiate_msg = WasmMsg::Instantiate2 {
            admin: None,
            code_id: factory_params.vesting_code_id,
            label: format!(
                "streamswap: Stream Addr {} Released to {}",
                env.contract.address, info.sender
            ),
            msg: to_json_binary(&vesting)?,
            funds: vec![coin(uint128_purchased.u128(), stream.out_asset.denom)],
            salt,
        };

        msgs.push(vesting_instantiate_msg.into());
        attrs.push(attr("vesting_address", address));
    } else {
        let send_msg = CosmosMsg::Bank(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: vec![Coin {
                denom: stream.out_asset.denom.to_string(),
                amount: uint128_purchased,
            }],
        });
        msgs.push(send_msg);
    }
    // if there is any unspent in balance, send it back to the user
    if !position.in_balance.is_zero() {
        let unspent = position.in_balance;
        let uint128_unspent = Uint128::try_from(unspent)?;
        let unspent_msg = CosmosMsg::Bank(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: vec![Coin {
                denom: stream.in_denom,
                amount: uint128_unspent,
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
        start_time: stream.status_info.start_time,
        end_time: stream.status_info.end_time,
        last_updated: stream.status_info.last_updated,
        spent_in: stream.spent_in,
        dist_index: stream.dist_index,
        out_remaining: stream.out_remaining,
        in_supply: stream.in_supply,
        shares: stream.shares,
        status: stream.status_info.status,
        url: stream.url,
        current_streamed_price: stream.current_streamed_price,
        stream_admin: stream.stream_admin.into_string(),
    };
    Ok(stream)
}

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
        last_updated: position.last_updated,
        pending_purchase: position.pending_purchase,
        exit_date: position.exit_date,
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
                last_updated: position.last_updated,
                pending_purchase: position.pending_purchase,
                exit_date: position.exit_date,
            };
            Ok(position)
        })
        .collect();
    let positions = positions?;
    Ok(PositionsResponse { positions })
}

pub fn query_average_price(deps: Deps, _env: Env) -> StdResult<AveragePriceResponse> {
    let stream = STREAM.load(deps.storage)?;
    let total_purchased = to_uint256(stream.out_asset.amount) - stream.out_remaining;
    let average_price = Decimal256::from_ratio(stream.spent_in, total_purchased);
    Ok(AveragePriceResponse { average_price })
}

pub fn query_last_streamed_price(deps: Deps, _env: Env) -> StdResult<LatestStreamedPriceResponse> {
    let stream = STREAM.load(deps.storage)?;
    Ok(LatestStreamedPriceResponse {
        current_streamed_price: stream.current_streamed_price,
    })
}

pub fn query_threshold_state(deps: Deps, _env: Env) -> Result<Option<Uint256>, StdError> {
    let threshold_state = ThresholdState::new();
    let threshold = threshold_state.get_threshold(deps.storage)?;
    Ok(threshold)
}
