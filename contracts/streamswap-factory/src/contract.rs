use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Coin, CosmosMsg, Decimal, Deps, DepsMut, Env, MessageInfo,
    Response, StdResult, WasmMsg,
};

use crate::{
    error::ContractError,
    msg::{CreateStreamMsg, ExecuteMsg, InstantiateMsg, QueryMsg},
    payment_checker::check_payment,
    state::{Params, FREEZESTATE, LAST_STREAM_ID, PARAMS},
};

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let InstantiateMsg {
        stream_swap_code_id,
        protocol_admin,
        stream_creation_fee,
        exit_fee_percent,
        accepted_in_denoms,
        fee_collector,
        min_stream_seconds,
        min_seconds_until_start_time,
        pool_creation_denom,
        vesting_code_id,
    } = msg;

    let protocol_admin = deps
        .api
        .addr_validate(&protocol_admin.unwrap_or(info.sender.to_string()))?;
    let fee_collector = deps
        .api
        .addr_validate(&fee_collector.unwrap_or(info.sender.to_string()))?;

    if exit_fee_percent > Decimal::percent(100) || exit_fee_percent < Decimal::percent(0) {
        return Err(ContractError::InvalidExitFeePercent {});
    }
    if stream_creation_fee.amount.is_zero() {
        return Err(ContractError::InvalidStreamCreationFee {});
    }

    let params = Params {
        stream_creation_fee: stream_creation_fee.clone(),
        exit_fee_percent,
        stream_swap_code_id,
        vesting_code_id,
        accepted_in_denoms,
        fee_collector,
        min_stream_seconds: min_stream_seconds.into(),
        min_seconds_until_start_time: min_seconds_until_start_time.into(),
        protocol_admin: protocol_admin.clone(),
    };
    PARAMS.save(deps.storage, &params)?;

    // Initilize Freezestate
    FREEZESTATE.save(deps.storage, &false)?;

    // Initilize Last Stream ID
    LAST_STREAM_ID.save(deps.storage, &0)?;

    let res = Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("admin", protocol_admin.to_string())
        .add_attribute(
            "stream_creation_fee",
            stream_creation_fee.amount.to_string(),
        )
        .add_attribute("exit_fee_percent", exit_fee_percent.to_string())
        .add_attribute("stream_swap_code_id", stream_swap_code_id.to_string());
    Ok(res)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateParams {
            min_stream_seconds,
            min_seconds_until_start_time,
            stream_creation_fee,
            fee_collector,
            accepted_in_denoms,
            exit_fee_percent,
        } => execute_update_params(
            deps,
            env,
            info,
            min_stream_seconds,
            min_seconds_until_start_time,
            stream_creation_fee,
            fee_collector,
            accepted_in_denoms,
            exit_fee_percent,
        ),
        ExecuteMsg::CreateStream { msg } => execute_create_stream(deps, env, info, msg),
        ExecuteMsg::Freeze {} => execute_freeze(deps, info),
    }
}
pub fn execute_create_stream(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: CreateStreamMsg,
) -> Result<Response, ContractError> {
    let is_frozen = FREEZESTATE.load(deps.storage)?;
    if is_frozen {
        return Err(ContractError::ContractIsFrozen {});
    }
    let CreateStreamMsg {
        treasury,
        name,
        out_asset,
        start_time,
        end_time,
        in_denom,
        stream_admin: _,
        threshold: _,
        url: _,
        create_pool: _,
        vesting,
    } = msg.clone();
    let params = PARAMS.load(deps.storage)?;
    let stream_creation_fee = params.stream_creation_fee.clone();
    let accepted_in_denoms = params.accepted_in_denoms.clone();
    if !accepted_in_denoms.contains(&in_denom) {
        return Err(ContractError::InDenomIsNotAccepted {});
    }
    let expected_funds = vec![stream_creation_fee.clone(), out_asset.clone()];
    check_payment(&info.funds, &expected_funds)?;
    let last_stream_id = LAST_STREAM_ID.load(deps.storage)?;
    let stream_id = last_stream_id + 1;

    if start_time > end_time {
        return Err(ContractError::StreamInvalidEndTime {});
    }
    if start_time < env.block.time {
        return Err(ContractError::StreamInvalidStartTime {});
    }
    // Explicitly handle the Option returned by checked_sub
    let stream_duration = end_time
        .seconds()
        .checked_sub(start_time.seconds())
        .ok_or(ContractError::StreamInvalidEndTime {})?;

    if stream_duration < params.min_stream_seconds {
        return Err(ContractError::StreamDurationTooShort {});
    }
    let time_until_start = start_time
        .seconds()
        .checked_sub(env.block.time.seconds())
        .ok_or(ContractError::StreamInvalidStartTime {})?;
    if time_until_start < params.min_seconds_until_start_time {
        return Err(ContractError::StreamStartsTooSoon {});
    }
    // let funds if out_asset is not 0
    let funds: Vec<Coin> = if out_asset.amount.is_zero() {
        vec![]
    } else {
        vec![out_asset.clone()]
    };

    let stream_swap_inst_message: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Instantiate {
        code_id: params.stream_swap_code_id,
        // TODO: discuss this
        admin: Some(params.protocol_admin.to_string()),
        label: format!("Stream Swap Stream {} - {}", name, stream_id),
        msg: to_json_binary(&msg)?,
        funds,
    });
    LAST_STREAM_ID.save(deps.storage, &stream_id)?;
    // TODO: If stream cration fee is zero this will fail
    let fund_transfer_message: CosmosMsg = CosmosMsg::Bank(cosmwasm_std::BankMsg::Send {
        to_address: params.fee_collector.to_string(),
        amount: vec![stream_creation_fee],
    });

    let res = Response::new()
        .add_message(stream_swap_inst_message)
        .add_message(fund_transfer_message)
        .add_attribute("action", "create_stream")
        .add_attribute("name", name)
        .add_attribute("treasury", treasury)
        .add_attribute("out_asset", out_asset.to_string())
        .add_attribute("start_time", start_time.to_string())
        .add_attribute("end time", end_time.to_string())
        .add_attribute("in_denom", in_denom);
    Ok(res)
}

pub fn execute_update_params(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    min_stream_seconds: Option<u64>,
    min_seconds_until_start_time: Option<u64>,
    stream_creation_fee: Option<Coin>,
    fee_collector: Option<String>,
    accepted_in_denoms: Option<Vec<String>>,
    exit_fee_percent: Option<Decimal>,
) -> Result<Response, ContractError> {
    let mut params = PARAMS.load(deps.storage)?;

    if info.sender != params.protocol_admin {
        return Err(ContractError::Unauthorized {});
    }
    if let Some(stream_creation_fee) = stream_creation_fee {
        params.stream_creation_fee = stream_creation_fee;
    }

    if let Some(exit_fee_percent) = exit_fee_percent {
        if exit_fee_percent > Decimal::percent(100) || exit_fee_percent < Decimal::percent(0) {
            return Err(ContractError::InvalidExitFeePercent {});
        }
        params.exit_fee_percent = exit_fee_percent;
    }

    if let Some(fee_collector) = fee_collector {
        params.fee_collector = deps.api.addr_validate(&fee_collector)?;
    }

    if let Some(accepted_in_denoms) = accepted_in_denoms {
        params.accepted_in_denoms = accepted_in_denoms;
    }

    if let Some(min_stream_seconds) = min_stream_seconds {
        params.min_stream_seconds = min_stream_seconds;
    }

    if let Some(min_seconds_until_start_time) = min_seconds_until_start_time {
        params.min_seconds_until_start_time = min_seconds_until_start_time;
    }

    PARAMS.save(deps.storage, &params)?;

    Ok(Response::new().add_attribute("action", "update_config"))
}

pub fn execute_freeze(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    let params = PARAMS.load(deps.storage)?;
    if info.sender != params.protocol_admin {
        return Err(ContractError::Unauthorized {});
    }
    FREEZESTATE.save(deps.storage, &true)?;
    Ok(Response::new().add_attribute("action", "freeze"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Params {} => to_json_binary(&PARAMS.load(deps.storage)?),
        QueryMsg::Freezestate {} => to_json_binary(&FREEZESTATE.load(deps.storage)?),
        QueryMsg::LastStreamId {} => to_json_binary(&LAST_STREAM_ID.load(deps.storage)?),
    }
}
