use crate::contract::{update_position, update_stream};
use crate::state::{Status, Stream, CONFIG, POSITIONS, STREAMS};
use crate::ContractError;
use cosmwasm_std::{
    attr, BankMsg, Coin, CosmosMsg, DepsMut, Env, MessageInfo, Response, StdResult, Uint128,
};
use cw_utils::maybe_addr;

pub fn execute_withdraw_paused(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    stream_id: u64,
    cap: Option<Uint128>,
    operator_target: Option<String>,
) -> Result<Response, ContractError> {
    let mut stream = STREAMS.load(deps.storage, stream_id)?;
    // check if stream is paused
    if !stream.is_paused() {
        return Err(ContractError::StreamNotPaused {});
    }
    // We are not checking if stream is ended because the paused state duration might exceed end time

    let operator_target =
        maybe_addr(deps.api, operator_target)?.unwrap_or_else(|| info.sender.clone());
    let mut position = POSITIONS.load(deps.storage, (stream_id, &operator_target))?;
    if position.owner != info.sender
        && position
            .operator
            .as_ref()
            .map_or(true, |o| o != &info.sender)
    {
        return Err(ContractError::Unauthorized {});
    }

    // on withdraw_paused we don't update_stream
    update_position(
        stream.dist_index,
        stream.shares,
        stream.last_updated_block,
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

    STREAMS.save(deps.storage, stream_id, &stream)?;
    POSITIONS.save(deps.storage, (stream_id, &position.owner), &position)?;

    let attributes = vec![
        attr("action", "withdraw_paused"),
        attr("stream_id", stream_id.to_string()),
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

pub fn execute_exit_cancelled(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    stream_id: u64,
    operator_target: Option<String>,
) -> Result<Response, ContractError> {
    let stream = STREAMS.load(deps.storage, stream_id)?;
    // check if stream is cancelled
    if !stream.is_cancelled() {
        return Err(ContractError::StreamNotCancelled {});
    }

    let operator_target =
        maybe_addr(deps.api, operator_target)?.unwrap_or_else(|| info.sender.clone());
    let position = POSITIONS.load(deps.storage, (stream_id, &operator_target))?;
    if position.owner != info.sender
        && position
            .operator
            .as_ref()
            .map_or(true, |o| o != &info.sender)
    {
        return Err(ContractError::Unauthorized {});
    }

    // no need to update position here, we just need to return total balance
    let total_balance = position.in_balance + position.spent;
    POSITIONS.remove(deps.storage, (stream_id, &position.owner));

    let attributes = vec![
        attr("action", "withdraw_cancelled"),
        attr("stream_id", stream_id.to_string()),
        attr("operator_target", operator_target.clone()),
        attr("total_balance", total_balance),
    ];

    // send funds to withdraw address or to the sender
    let res = Response::new()
        .add_message(CosmosMsg::Bank(BankMsg::Send {
            to_address: operator_target.to_string(),
            amount: vec![Coin {
                denom: stream.in_denom,
                amount: total_balance,
            }],
        }))
        .add_attributes(attributes);

    Ok(res)
}

pub fn execute_pause_stream(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    stream_id: u64,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.protocol_admin {
        return Err(ContractError::Unauthorized {});
    }
    //check if stream is ended
    let stream = STREAMS.load(deps.storage, stream_id)?;
    if env.block.height >= stream.end_block {
        return Err(ContractError::StreamEnded {});
    }
    // check if stream is not started
    if env.block.height < stream.start_block {
        return Err(ContractError::StreamNotStarted {});
    }
    // paused or cancelled can not be paused
    if stream.is_killswitch_active() {
        return Err(ContractError::StreamKillswitchActive {});
    }
    // update stream before pause
    let mut stream = STREAMS.load(deps.storage, stream_id)?;
    update_stream(env.block.height, &mut stream)?;
    pause_stream(env.block.height, &mut stream)?;
    STREAMS.save(deps.storage, stream_id, &stream)?;

    Ok(Response::default()
        .add_attribute("action", "pause_stream")
        .add_attribute("stream_id", stream_id.to_string())
        .add_attribute("is_paused", "true")
        .add_attribute("pause_block", env.block.height.to_string()))
}

pub fn pause_stream(now_block: u64, stream: &mut Stream) -> StdResult<()> {
    stream.status = Status::Paused;
    stream.pause_block = Some(now_block);
    Ok(())
}

pub fn execute_resume_stream(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    stream_id: u64,
) -> Result<Response, ContractError> {
    let mut stream = STREAMS.load(deps.storage, stream_id)?;
    let cfg = CONFIG.load(deps.storage)?;
    //Cancelled can't be resumed
    if stream.is_cancelled() {
        return Err(ContractError::StreamIsCancelled {});
    }
    if stream.status != Status::Paused {
        return Err(ContractError::StreamNotPaused {});
    }
    if cfg.protocol_admin != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    let pause_block = stream.pause_block.unwrap();
    //postpone stream times with respect to pause duration
    stream.end_block = stream.end_block + (env.block.height - pause_block);
    stream.last_updated_block = stream.last_updated_block + (env.block.height - pause_block);

    stream.status = Status::Active;
    STREAMS.save(deps.storage, stream_id, &stream)?;

    let attributes = vec![
        attr("action", "resume_stream"),
        attr("stream_id", stream_id.to_string()),
    ];
    Ok(Response::default().add_attributes(attributes))
}

pub fn execute_cancel_stream(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    stream_id: u64,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    if cfg.protocol_admin != info.sender {
        return Err(ContractError::Unauthorized {});
    }
    let mut stream = STREAMS.load(deps.storage, stream_id)?;
    if stream.is_cancelled() {
        return Err(ContractError::StreamIsCancelled {});
    }
    if !stream.is_paused() {
        return Err(ContractError::StreamNotPaused {});
    }
    stream.status = Status::Cancelled;
    STREAMS.save(deps.storage, stream_id, &stream)?;

    //Refund all out tokens to stream creator(treasury)
    let messages: Vec<CosmosMsg> = vec![
        CosmosMsg::Bank(BankMsg::Send {
            to_address: stream.treasury.to_string(),
            amount: vec![Coin {
                denom: stream.out_denom,
                amount: stream.out_supply,
            }],
        }),
        //Refund stream creation fee to stream creator
        CosmosMsg::Bank(BankMsg::Send {
            to_address: stream.treasury.to_string(),
            amount: vec![Coin {
                denom: stream.stream_creation_denom,
                amount: stream.stream_creation_fee,
            }],
        }),
    ];

    Ok(Response::new()
        .add_attribute("action", "cancel_stream")
        .add_messages(messages)
        .add_attribute("stream_id", stream_id.to_string())
        .add_attribute("status", "cancelled"))
}

pub fn execute_threshold_cancel_stream(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    stream_id: u64,
) -> Result<Response, ContractError> {
    let mut stream = STREAMS.load(deps.storage, stream_id)?;

    // check if stream is ended
    if env.block.height >= stream.end_block {
        return Err(ContractError::StreamEnded {});
    }
    update_stream(env.block.height, &mut stream)?;

    if let Some(target_price) = stream.target_price {
        if stream.current_streamed_price > target_price {
            return Err(ContractError::StreamThresholdPriceNotMet {});
        }
    } else {
        return Err(ContractError::StreamThresholdPriceNotSet {});
    }

    stream.status = Status::ThresholdNotMet;
    STREAMS.save(deps.storage, stream_id, &stream)?;

    //Refund all out tokens to stream creator(treasury)
    let messages: Vec<CosmosMsg> = vec![
        CosmosMsg::Bank(BankMsg::Send {
            to_address: stream.treasury.to_string(),
            amount: vec![Coin {
                denom: stream.out_denom,
                amount: stream.out_supply,
            }],
        }),
        //Refund stream creation fee to stream creator
        CosmosMsg::Bank(BankMsg::Send {
            to_address: stream.treasury.to_string(),
            amount: vec![Coin {
                denom: stream.stream_creation_denom,
                amount: stream.stream_creation_fee,
            }],
        }),
    ];

    Ok(Response::new()
        .add_attribute("action", "threshold_cancel_stream")
        .add_messages(messages)
        .add_attribute("stream_id", stream_id.to_string())
        .add_attribute("status", "cancelled"))
}

pub fn sudo_pause_stream(
    deps: DepsMut,
    env: Env,
    stream_id: u64,
) -> Result<Response, ContractError> {
    let mut stream = STREAMS.load(deps.storage, stream_id)?;

    if env.block.height >= stream.end_block {
        return Err(ContractError::StreamEnded {});
    }
    // check if stream is not started
    if env.block.height < stream.start_block {
        return Err(ContractError::StreamNotStarted {});
    }
    // Paused or cancelled can not be paused
    if stream.is_killswitch_active() {
        return Err(ContractError::StreamKillswitchActive {});
    }
    update_stream(env.block.height, &mut stream)?;
    pause_stream(env.block.height, &mut stream)?;
    STREAMS.save(deps.storage, stream_id, &stream)?;

    Ok(Response::default()
        .add_attribute("action", "sudo_pause_stream")
        .add_attribute("stream_id", stream_id.to_string())
        .add_attribute("is_paused", "true")
        .add_attribute("pause_block", env.block.height.to_string()))
}

pub fn sudo_resume_stream(
    deps: DepsMut,
    env: Env,
    stream_id: u64,
) -> Result<Response, ContractError> {
    let mut stream = STREAMS.load(deps.storage, stream_id)?;
    //Cancelled can't be resumed
    if stream.is_cancelled() {
        return Err(ContractError::StreamIsCancelled {});
    }
    //Only paused can be resumed
    if !stream.is_paused() {
        return Err(ContractError::StreamNotPaused {});
    }
    // ok to use unwrap here
    let pause_block = stream.pause_block.unwrap();
    //postpone stream times with respect to pause duration
    stream.end_block = stream.end_block + (env.block.height - pause_block);
    stream.last_updated_block = stream.last_updated_block + (env.block.height - pause_block);

    stream.status = Status::Active;
    stream.pause_block = None;
    STREAMS.save(deps.storage, stream_id, &stream)?;

    Ok(Response::default()
        .add_attribute("action", "resume_stream")
        .add_attribute("stream_id", stream_id.to_string())
        .add_attribute("new_end_date", stream.end_block.to_string())
        .add_attribute("status", "active"))
}

pub fn sudo_cancel_stream(
    deps: DepsMut,
    _env: Env,
    stream_id: u64,
) -> Result<Response, ContractError> {
    let mut stream = STREAMS.load(deps.storage, stream_id)?;
    if stream.is_cancelled() {
        return Err(ContractError::StreamIsCancelled {});
    }
    if !stream.is_paused() {
        return Err(ContractError::StreamNotPaused {});
    }
    stream.status = Status::Cancelled;
    STREAMS.save(deps.storage, stream_id, &stream)?;

    //Refund all out tokens to stream creator(treasury)
    let messages: Vec<CosmosMsg> = vec![
        CosmosMsg::Bank(BankMsg::Send {
            to_address: stream.treasury.to_string(),
            amount: vec![Coin {
                denom: stream.out_denom,
                amount: stream.out_supply,
            }],
        }),
        //Refund stream creation fee to stream creator
        CosmosMsg::Bank(BankMsg::Send {
            to_address: stream.treasury.to_string(),
            amount: vec![Coin {
                denom: stream.stream_creation_denom,
                amount: stream.stream_creation_fee,
            }],
        }),
    ];

    Ok(Response::new()
        .add_attribute("action", "cancel_stream")
        .add_messages(messages)
        .add_attribute("stream_id", stream_id.to_string())
        .add_attribute("status", "cancelled"))
}
