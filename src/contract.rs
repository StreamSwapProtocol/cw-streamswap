use crate::msg::{
    AveragePriceResponse, ConfigResponse, ExecuteMsg, InstantiateMsg, LatestStreamedPriceResponse,
    MigrateMsg, PositionResponse, PositionsResponse, PriceResponse, QueryMsg, StreamResponse,
    StreamsResponse, SudoMsg,
};
use crate::state::{next_stream_id, Config, Position, Status, Stream, CONFIG, POSITIONS, STREAMS};
use crate::{killswitch, sudo, ContractError};
use cosmwasm_std::{
    attr, entry_point, to_binary, Addr, BankMsg, Binary, Coin, CosmosMsg, Decimal, Decimal256,
    Deps, DepsMut, Env, Fraction, MessageInfo, Order, Response, StdResult, Uint128, Uint256,
};
use cw2::{get_contract_version, set_contract_version};
use semver::Version;

use crate::helpers::{check_name_and_url, from_semver, get_decimals};
use cw_storage_plus::Bound;
use cw_utils::{maybe_addr, must_pay};

// Version and contract info for migration
const CONTRACT_NAME: &str = "crates.io:cw-streamswap";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    // exit fee percent can not be equal to or greater than 1, or smaller than 0
    if msg.exit_fee_percent >= Decimal::one() || msg.exit_fee_percent < Decimal::zero() {
        return Err(ContractError::InvalidExitFeePercent {});
    }

    if msg.stream_creation_fee.is_zero() {
        return Err(ContractError::InvalidStreamCreationFee {});
    }

    let config = Config {
        min_stream_blocks: msg.min_stream_blocks,
        min_blocks_until_start_block: msg.min_blocks_until_start_block,
        stream_creation_denom: msg.stream_creation_denom.clone(),
        stream_creation_fee: msg.stream_creation_fee,
        exit_fee_percent: msg.exit_fee_percent,
        fee_collector: deps.api.addr_validate(&msg.fee_collector)?,
        protocol_admin: deps.api.addr_validate(&msg.protocol_admin)?,
        accepted_in_denom: msg.accepted_in_denom,
        oracle_contract: deps.api.addr_validate(&msg.oracle_contract)?,
    };
    CONFIG.save(deps.storage, &config)?;

    let attrs = vec![
        attr("action", "instantiate"),
        attr("min_stream_blocks", msg.min_stream_blocks.to_string()),
        attr(
            "min_blocks_until_start_block",
            msg.min_blocks_until_start_block.to_string(),
        ),
        attr("stream_creation_denom", msg.stream_creation_denom),
        attr("stream_creation_fee", msg.stream_creation_fee),
        attr("exit_fee_percent", msg.exit_fee_percent.to_string()),
        attr("fee_collector", msg.fee_collector),
        attr("protocol_admin", msg.protocol_admin),
    ];
    Ok(Response::default().add_attributes(attrs))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::CreateStream {
            treasury,
            name,
            url,
            in_denom,
            out_denom,
            out_supply,
            start_block,
            end_block,
        } => execute_create_stream(
            deps,
            env,
            info,
            treasury,
            name,
            url,
            in_denom,
            out_denom,
            out_supply,
            start_block,
            end_block,
        ),
        ExecuteMsg::UpdateOperator {
            stream_id,
            new_operator,
        } => execute_update_operator(deps, env, info, stream_id, new_operator),
        ExecuteMsg::UpdatePosition {
            stream_id,
            operator_target,
        } => execute_update_position(deps, env, info, stream_id, operator_target),
        ExecuteMsg::UpdateStream { stream_id } => execute_update_stream(deps, env, stream_id),
        ExecuteMsg::Subscribe {
            stream_id,
            operator_target,
            operator,
        } => {
            let stream = STREAMS.load(deps.storage, stream_id)?;
            execute_subscribe(
                deps,
                env,
                info,
                stream_id,
                operator,
                operator_target,
                stream,
            )
        }
        ExecuteMsg::Withdraw {
            stream_id,
            cap,
            operator_target,
        } => {
            let stream = STREAMS.load(deps.storage, stream_id)?;
            execute_withdraw(deps, env, info, stream_id, stream, cap, operator_target)
        }
        ExecuteMsg::FinalizeStream {
            stream_id,
            new_treasury,
        } => execute_finalize_stream(deps, env, info, stream_id, new_treasury),
        ExecuteMsg::ExitStream {
            stream_id,
            operator_target,
        } => execute_exit_stream(deps, env, info, stream_id, operator_target),

        ExecuteMsg::PauseStream { stream_id } => {
            killswitch::execute_pause_stream(deps, env, info, stream_id)
        }
        ExecuteMsg::ResumeStream { stream_id } => {
            killswitch::execute_resume_stream(deps, env, info, stream_id)
        }
        ExecuteMsg::CancelStream { stream_id } => {
            killswitch::execute_cancel_stream(deps, env, info, stream_id)
        }
        ExecuteMsg::WithdrawPaused {
            stream_id,
            cap,
            operator_target,
        } => killswitch::execute_withdraw_paused(deps, env, info, stream_id, cap, operator_target),
        ExecuteMsg::ExitCancelled {
            stream_id,
            operator_target,
        } => killswitch::execute_exit_cancelled(deps, env, info, stream_id, operator_target),
        ExecuteMsg::UpdateProtocolAdmin {
            new_protocol_admin: new_admin,
        } => execute_update_protocol_admin(deps, env, info, new_admin),
        ExecuteMsg::UpdateConfig {
            min_stream_blocks,
            min_blocks_until_start_block,
            stream_creation_denom,
            stream_creation_fee,
            fee_collector,
            accepted_in_denom,
            exit_fee_percent,
            oracle_contract,
        } => execute_update_config(
            deps,
            env,
            info,
            min_stream_blocks,
            min_blocks_until_start_block,
            stream_creation_denom,
            stream_creation_fee,
            fee_collector,
            accepted_in_denom,
            exit_fee_percent,
            oracle_contract,
        ),
        // ExecuteMsg::TestOracle {} => test_oracle(deps, env),
    }
}
#[allow(clippy::too_many_arguments)]
pub fn execute_create_stream(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    treasury: String,
    name: String,
    url: Option<String>,
    in_denom: String,
    out_denom: String,
    out_supply: Uint128,
    start_block: u64,
    end_block: u64,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if end_block <= start_block {
        return Err(ContractError::StreamInvalidEndBlock {});
    }
    if env.block.height > start_block {
        return Err(ContractError::StreamInvalidStartBlock {});
    }
    if end_block - start_block < config.min_stream_blocks {
        return Err(ContractError::StreamDurationTooShort {});
    }

    if start_block - env.block.height < config.min_blocks_until_start_block {
        return Err(ContractError::StreamStartsTooSoon {});
    }

    if in_denom != config.accepted_in_denom {
        return Err(ContractError::InDenomIsNotAccepted {});
    }

    if in_denom == out_denom {
        return Err(ContractError::SameDenomOnEachSide {});
    }

    if out_supply < Uint128::from(1u128) {
        return Err(ContractError::ZeroOutSupply {});
    }

    if out_denom == config.stream_creation_denom {
        let total_funds = info
            .funds
            .iter()
            .find(|p| p.denom == config.stream_creation_denom)
            .ok_or(ContractError::NoFundsSent {})?;

        if total_funds.amount != config.stream_creation_fee + out_supply {
            return Err(ContractError::StreamOutSupplyFundsRequired {});
        }
        // check for extra funds sent in msg
        if info.funds.iter().any(|p| p.denom != out_denom) {
            return Err(ContractError::InvalidFunds {});
        }
    } else {
        let funds = info
            .funds
            .iter()
            .find(|p| p.denom == out_denom)
            .ok_or(ContractError::NoFundsSent {})?;

        if funds.amount != out_supply {
            return Err(ContractError::StreamOutSupplyFundsRequired {});
        }

        let creation_fee = info
            .funds
            .iter()
            .find(|p| p.denom == config.stream_creation_denom)
            .ok_or(ContractError::NoFundsSent {})?;
        if creation_fee.amount != config.stream_creation_fee {
            return Err(ContractError::StreamCreationFeeRequired {});
        }

        if info
            .funds
            .iter()
            .any(|p| p.denom != out_denom && p.denom != config.stream_creation_denom)
        {
            return Err(ContractError::InvalidFunds {});
        }
    }

    check_name_and_url(&name, &url)?;

    let stream = Stream::new(
        name.clone(),
        deps.api.addr_validate(&treasury)?,
        url.clone(),
        out_denom.clone(),
        out_supply,
        in_denom.clone(),
        start_block,
        end_block,
        // Last updated block is set to start block.
        start_block,
        config.stream_creation_denom,
        config.stream_creation_fee,
        config.exit_fee_percent,
        info.sender,
    );
    let id = next_stream_id(deps.storage)?;
    STREAMS.save(deps.storage, id, &stream)?;

    let attr = vec![
        attr("action", "create_stream"),
        attr("id", id.to_string()),
        attr("treasury", treasury),
        attr("name", name),
        attr("url", url.unwrap_or_default()),
        attr("in_denom", in_denom),
        attr("out_denom", out_denom),
        attr("out_supply", out_supply),
        attr("start_block", start_block.to_string()),
        attr("end_block", end_block.to_string()),
    ];
    Ok(Response::default().add_attributes(attr))
}

pub fn execute_update_protocol_admin(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    new_admin: String,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.protocol_admin {
        return Err(ContractError::Unauthorized {});
    }
    config.protocol_admin = deps.api.addr_validate(&new_admin)?;
    CONFIG.save(deps.storage, &config)?;

    let attrs = vec![
        attr("action", "update_protocol_admin"),
        attr("new_admin", new_admin),
    ];

    Ok(Response::default().add_attributes(attrs))
}

/// Updates stream to calculate released distribution and spent amount
pub fn execute_update_stream(
    deps: DepsMut,
    env: Env,
    stream_id: u64,
) -> Result<Response, ContractError> {
    let mut stream = STREAMS.load(deps.storage, stream_id)?;

    if stream.is_paused() {
        return Err(ContractError::StreamPaused {});
    }
    let (_, dist_amount) = update_stream(env.block.height, &mut stream)?;
    STREAMS.save(deps.storage, stream_id, &stream)?;

    let attrs = vec![
        attr("action", "update_stream"),
        attr("stream_id", stream_id.to_string()),
        attr("new_distribution_amount", dist_amount),
        attr("dist_index", stream.dist_index.to_string()),
    ];
    let res = Response::new().add_attributes(attrs);
    Ok(res)
}

pub fn update_stream(
    now_block: u64,
    stream: &mut Stream,
) -> Result<(Decimal, Uint128), ContractError> {
    let diff = calculate_diff(stream.end_block, stream.last_updated_block, now_block);

    let mut new_distribution_balance = Uint128::zero();

    // if no in balance in the contract, no need to update
    // if diff not changed this means either stream not started or no in balance so far
    if !stream.shares.is_zero() && !diff.is_zero() {
        // new distribution balance is the amount of in tokens that has been distributed since last update
        // distribution is linear for now.
        new_distribution_balance = stream
            .out_remaining
            .multiply_ratio(diff.numerator(), diff.denominator());
        // spent in tokens is the amount of in tokens that has been spent since last update
        // spending is linear and goes to zero at the end of the stream
        let spent_in = stream
            .in_supply
            .multiply_ratio(diff.numerator(), diff.denominator());

        // increase total spent_in of the stream
        stream.spent_in = stream.spent_in.checked_add(spent_in)?;
        // decrease in_supply of the steam
        stream.in_supply = stream.in_supply.checked_sub(spent_in)?;

        // if no new distribution balance, no need to update the price, out_remaining and dist_index
        if !new_distribution_balance.is_zero() {
            // decrease amount to be distributed of the stream
            stream.out_remaining = stream.out_remaining.checked_sub(new_distribution_balance)?;
            // update distribution index. A positions share of the distribution is calculated by
            // multiplying the share by the distribution index
            stream.dist_index = stream.dist_index.checked_add(Decimal256::from_ratio(
                new_distribution_balance,
                stream.shares,
            ))?;
            stream.current_streamed_price = Decimal::from_ratio(spent_in, new_distribution_balance)
        }
    }
    // Last updated block is set to now_block or start_block if now_block < start_block
    // This means stream is updated before start time.
    // In this case Last updated block is set to start_block(This is default because at create stream last_updated_block is set to start_block)
    // Also if now is greater than start block and status is waiting, status is set to active
    stream.last_updated_block = if now_block < stream.start_block {
        stream.start_block
    } else {
        if stream.status == Status::Waiting {
            stream.status = Status::Active
        }
        now_block
    };

    Ok((diff, new_distribution_balance))
}

fn calculate_diff(end_block: u64, last_updated_block: u64, now_block: u64) -> Decimal {
    // diff = (now_block - last_updated_block) / (end_block - last_updated_block)
    let now_block = if now_block > end_block {
        end_block
    } else {
        now_block
    };
    let numerator = now_block.saturating_sub(last_updated_block);
    let denominator = end_block.saturating_sub(last_updated_block);

    if denominator == 0 || numerator == 0 {
        Decimal::zero()
    } else {
        Decimal::from_ratio(numerator, denominator)
    }
}

pub fn execute_update_position(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    stream_id: u64,
    operator_target: Option<String>,
) -> Result<Response, ContractError> {
    let operator_target =
        maybe_addr(deps.api, operator_target)?.unwrap_or_else(|| info.sender.clone());
    let mut position = POSITIONS.load(deps.storage, (stream_id, &operator_target))?;
    check_access(&info, &position.owner, &position.operator)?;

    let mut stream = STREAMS.load(deps.storage, stream_id)?;
    // check if stream is paused
    if stream.is_paused() {
        return Err(ContractError::StreamPaused {});
    }

    // sync stream
    update_stream(env.block.height, &mut stream)?;
    STREAMS.save(deps.storage, stream_id, &stream)?;

    // updates position to latest distribution. Returns the amount of out tokens that has been purchased
    // and in tokens that has been spent.
    let (purchased, spent) = update_position(
        stream.dist_index,
        stream.shares,
        stream.last_updated_block,
        stream.in_supply,
        &mut position,
    )?;
    POSITIONS.save(deps.storage, (stream_id, &position.owner), &position)?;

    Ok(Response::new()
        .add_attribute("action", "update_position")
        .add_attribute("stream_id", stream_id.to_string())
        .add_attribute("operator_target", operator_target)
        .add_attribute("purchased", purchased)
        .add_attribute("spent", spent))
}

// calculate the user purchase based on the positions index and the global index.
// returns purchased out amount and spent in amount
pub fn update_position(
    stream_dist_index: Decimal256,
    stream_shares: Uint128,
    stream_last_updated_block: u64,
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
    position.last_updated_block = stream_last_updated_block;

    Ok((purchased_uint128, spent))
}

pub fn execute_subscribe(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    stream_id: u64,
    operator: Option<String>,
    operator_target: Option<String>,
    mut stream: Stream,
) -> Result<Response, ContractError> {
    // check if stream is paused
    if stream.is_killswitch_active() {
        return Err(ContractError::StreamKillswitchActive {});
    }

    if env.block.height >= stream.end_block {
        return Err(ContractError::StreamEnded {});
    }

    let in_amount = must_pay(&info, &stream.in_denom)?;
    let new_shares;

    let operator = maybe_addr(deps.api, operator)?;
    let operator_target =
        maybe_addr(deps.api, operator_target)?.unwrap_or_else(|| info.sender.clone());
    let position = POSITIONS.may_load(deps.storage, (stream_id, &operator_target))?;
    match position {
        None => {
            // operator cannot create a position in behalf of anyone
            if operator_target != info.sender {
                return Err(ContractError::Unauthorized {});
            }
            update_stream(env.block.height, &mut stream)?;
            new_shares = stream.compute_shares_amount(in_amount, false);
            // new positions do not update purchase as it has no effect on distribution
            let new_position = Position::new(
                info.sender,
                in_amount,
                new_shares,
                Some(stream.dist_index),
                env.block.height,
                operator,
            );
            POSITIONS.save(deps.storage, (stream_id, &operator_target), &new_position)?;
        }
        Some(mut position) => {
            check_access(&info, &position.owner, &position.operator)?;

            // incoming tokens should not participate in prev distribution
            update_stream(env.block.height, &mut stream)?;
            new_shares = stream.compute_shares_amount(in_amount, false);
            update_position(
                stream.dist_index,
                stream.shares,
                stream.last_updated_block,
                stream.in_supply,
                &mut position,
            )?;

            position.in_balance = position.in_balance.checked_add(in_amount)?;
            position.shares = position.shares.checked_add(new_shares)?;
            POSITIONS.save(deps.storage, (stream_id, &operator_target), &position)?;
        }
    }

    // increase in supply and shares
    stream.in_supply = stream.in_supply.checked_add(in_amount)?;
    stream.shares = stream.shares.checked_add(new_shares)?;
    STREAMS.save(deps.storage, stream_id, &stream)?;

    let res = Response::new()
        .add_attribute("action", "subscribe")
        .add_attribute("stream_id", stream_id.to_string())
        .add_attribute("owner", operator_target)
        .add_attribute("in_supply", stream.in_supply)
        .add_attribute("in_amount", in_amount);

    Ok(res)
}

// pub fn execute_subscribe_pending(
//     deps: DepsMut,
//     env: Env,
//     info: MessageInfo,
//     stream_id: u64,
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
//     let position = POSITIONS.may_load(deps.storage, (stream_id, &operator_target))?;
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
//                 env.block.height,
//                 operator,
//             );
//             POSITIONS.save(deps.storage, (stream_id, &operator_target), &new_position)?;
//         }
//         Some(mut position) => {
//             check_access(&info, &position.owner, &position.operator)?;
//             // if subscibed already, we wont update its position but just increase its in_balance and shares
//             position.in_balance = position.in_balance.checked_add(in_amount)?;
//             position.shares = position.shares.checked_add(new_shares)?;
//             POSITIONS.save(deps.storage, (stream_id, &operator_target), &position)?;
//         }
//     }
//     stream.in_supply = stream.in_supply.checked_add(in_amount)?;
//     stream.shares = stream.shares.checked_add(new_shares)?;
//     STREAMS.save(deps.storage, stream_id, &stream)?;

//     Ok(Response::new()
//         .add_attribute("action", "subscribe_pending")
//         .add_attribute("stream_id", stream_id.to_string())
//         .add_attribute("owner", operator_target)
//         .add_attribute("in_supply", stream.in_supply)
//         .add_attribute("in_amount", in_amount))
// }

pub fn execute_update_operator(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    stream_id: u64,
    operator: Option<String>,
) -> Result<Response, ContractError> {
    let mut position = POSITIONS.load(deps.storage, (stream_id, &info.sender))?;

    let operator = maybe_addr(deps.api, operator)?;
    position.operator = operator.clone();

    POSITIONS.save(deps.storage, (stream_id, &info.sender), &position)?;

    Ok(Response::new()
        .add_attribute("action", "update_operator")
        .add_attribute("stream_id", stream_id.to_string())
        .add_attribute("owner", info.sender)
        .add_attribute("operator", operator.unwrap_or_else(|| Addr::unchecked(""))))
}

pub fn execute_withdraw(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    stream_id: u64,
    mut stream: Stream,
    cap: Option<Uint128>,
    operator_target: Option<String>,
) -> Result<Response, ContractError> {
    // check if stream is paused
    if stream.is_killswitch_active() {
        return Err(ContractError::StreamKillswitchActive {});
    }
    // can't withdraw after stream ended
    if env.block.height >= stream.end_block {
        return Err(ContractError::StreamEnded {});
    }

    let operator_target =
        maybe_addr(deps.api, operator_target)?.unwrap_or_else(|| info.sender.clone());
    let mut position = POSITIONS.load(deps.storage, (stream_id, &operator_target))?;
    check_access(&info, &position.owner, &position.operator)?;

    update_stream(env.block.height, &mut stream)?;
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
        attr("action", "withdraw"),
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

// pub fn execute_withdraw_pending(
//     deps: DepsMut,
//     _env: Env,
//     info: MessageInfo,
//     stream_id: u64,
//     mut stream: Stream,
//     cap: Option<Uint128>,
//     operator_target: Option<String>,
// ) -> Result<Response, ContractError> {
//     // check if stream is paused
//     let operator_target =
//         maybe_addr(deps.api, operator_target)?.unwrap_or_else(|| info.sender.clone());
//     let mut position = POSITIONS.load(deps.storage, (stream_id, &operator_target))?;
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

//     STREAMS.save(deps.storage, stream_id, &stream)?;
//     POSITIONS.save(deps.storage, (stream_id, &position.owner), &position)?;

//     let attributes = vec![
//         attr("action", "withdraw_pending"),
//         attr("stream_id", stream_id.to_string()),
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
    stream_id: u64,
    new_treasury: Option<String>,
) -> Result<Response, ContractError> {
    let mut stream = STREAMS.load(deps.storage, stream_id)?;
    // check if the stream is already finalized
    if stream.status == Status::Finalized {
        return Err(ContractError::StreamAlreadyFinalized {});
    }
    // check if killswitch is active
    if stream.is_killswitch_active() {
        return Err(ContractError::StreamKillswitchActive {});
    }
    if stream.treasury != info.sender {
        return Err(ContractError::Unauthorized {});
    }
    if env.block.height <= stream.end_block {
        return Err(ContractError::StreamNotEnded {});
    }
    if stream.last_updated_block < stream.end_block {
        update_stream(env.block.height, &mut stream)?;
    }

    if stream.status == Status::Active {
        stream.status = Status::Finalized
    }
    STREAMS.save(deps.storage, stream_id, &stream)?;

    let config = CONFIG.load(deps.storage)?;
    let treasury = maybe_addr(deps.api, new_treasury)?.unwrap_or_else(|| stream.treasury.clone());
    // We are awaiting a price response from the oracle contract
    // If no response is received, no discount is applied
    let in_denom_price_response: PriceResponse = deps
        .querier
        .query_wasm_smart(
            config.oracle_contract.clone(),
            &QueryMsg::Price {
                denom: config.accepted_in_denom.clone(),
            },
        )
        .unwrap_or(PriceResponse {
            price: Decimal::zero(),
            denom: "".to_string(),
        });
    let in_denom_price = in_denom_price_response.price;
    let total_revenue_in_usd = Decimal::from_ratio(stream.spent_in, Uint128::one())
        .checked_mul(in_denom_price)?
        * Uint128::one();
    // Every 100_000 USD of total revenue collected, the swap fee percent discount is increased by 0.001 up to 0.02
    // If say total revenue is 500_000 USD, then swap fee discount is Decimal 0.005
    let swap_fee_discount = Decimal::from_ratio(
        total_revenue_in_usd.checked_div(Uint128::from(100_000u128))?,
        Uint128::one(),
    )
    .checked_mul(Decimal::from_ratio(1u128, 1000u128))?
    .min(Decimal::percent(2));

    let final_swap_fee_percent = stream
        .stream_exit_fee_percent
        .checked_sub(swap_fee_discount)?;

    //Stream's swap fee collected at dynamic rate at finalize
    let swap_fee = Decimal::from_ratio(stream.spent_in, Uint128::one())
        .checked_mul(final_swap_fee_percent)?
        * Uint128::one();

    let creator_revenue = stream.spent_in.checked_sub(swap_fee)?;

    //Creator's revenue claimed at finalize
    let revenue_msg = CosmosMsg::Bank(BankMsg::Send {
        to_address: treasury.to_string(),
        amount: vec![Coin {
            denom: stream.in_denom.clone(),
            amount: creator_revenue,
        }],
    });
    //Exact fee for stream creation charged at creation but claimed at finalize
    let creation_fee_msg = CosmosMsg::Bank(BankMsg::Send {
        to_address: config.fee_collector.to_string(),
        amount: vec![Coin {
            denom: stream.stream_creation_denom,
            amount: stream.stream_creation_fee,
        }],
    });

    let swap_fee_msg = CosmosMsg::Bank(BankMsg::Send {
        to_address: config.fee_collector.to_string(),
        amount: vec![Coin {
            denom: stream.in_denom,
            amount: swap_fee,
        }],
    });

    let mut messages = if stream.spent_in != Uint128::zero() {
        vec![revenue_msg, creation_fee_msg, swap_fee_msg]
    } else {
        vec![creation_fee_msg]
    };

    // In case the stream is ended without any shares in it. We need to refund the remaining out tokens although that is unlikely to happen
    if stream.out_remaining > Uint128::zero() {
        let remaining_out = stream.out_remaining;
        let remaining_msg = CosmosMsg::Bank(BankMsg::Send {
            to_address: treasury.to_string(),
            amount: vec![Coin {
                denom: stream.out_denom,
                amount: remaining_out,
            }],
        });
        messages.push(remaining_msg);
    }

    Ok(Response::new().add_messages(messages).add_attributes(vec![
        attr("action", "finalize_stream"),
        attr("stream_id", stream_id.to_string()),
        attr("treasury", treasury.as_str()),
        attr("fee_collector", config.fee_collector.to_string()),
        attr("creators_revenue", creator_revenue),
        attr("refunded_out_remaining", stream.out_remaining.to_string()),
        attr(
            "total_sold",
            stream
                .out_supply
                .checked_sub(stream.out_remaining)?
                .to_string(),
        ),
        attr("swap_fee", swap_fee),
        attr("creation_fee", config.stream_creation_fee.to_string()),
    ]))
}

pub fn execute_exit_stream(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    stream_id: u64,
    operator_target: Option<String>,
) -> Result<Response, ContractError> {
    let mut stream = STREAMS.load(deps.storage, stream_id)?;
    let _config = CONFIG.load(deps.storage)?;
    // check if stream is paused
    if stream.is_killswitch_active() {
        return Err(ContractError::StreamKillswitchActive {});
    }
    if env.block.height <= stream.end_block {
        return Err(ContractError::StreamNotEnded {});
    }
    if stream.last_updated_block < stream.end_block {
        update_stream(env.block.height, &mut stream)?;
    }
    let operator_target =
        maybe_addr(deps.api, operator_target)?.unwrap_or_else(|| info.sender.clone());
    let mut position = POSITIONS.load(deps.storage, (stream_id, &operator_target))?;
    check_access(&info, &position.owner, &position.operator)?;

    // update position before exit
    update_position(
        stream.dist_index,
        stream.shares,
        stream.last_updated_block,
        stream.in_supply,
        &mut position,
    )?;
    // Swap fee = fixed_rate*position.spent_in this calculation is only for execution reply attributes
    let swap_fee = Decimal::from_ratio(position.spent, Uint128::one())
        .checked_mul(stream.stream_exit_fee_percent)?
        * Uint128::one();

    let send_msg = CosmosMsg::Bank(BankMsg::Send {
        to_address: operator_target.to_string(),
        amount: vec![Coin {
            denom: stream.out_denom.to_string(),
            amount: position.purchased,
        }],
    });

    stream.shares = stream.shares.checked_sub(position.shares)?;

    STREAMS.save(deps.storage, stream_id, &stream)?;
    POSITIONS.remove(deps.storage, (stream_id, &position.owner));

    let attributes = vec![
        attr("action", "exit_stream"),
        attr("stream_id", stream_id.to_string()),
        attr("spent", position.spent.checked_sub(swap_fee)?),
        attr("purchased", position.purchased),
        attr("swap_fee_paid", swap_fee),
    ];
    if !position.in_balance.is_zero() {
        let unspent = position.in_balance;
        let unspent_msg = CosmosMsg::Bank(BankMsg::Send {
            to_address: operator_target.to_string(),
            amount: vec![Coin {
                denom: stream.in_denom,
                amount: unspent,
            }],
        });

        Ok(Response::new()
            .add_message(send_msg)
            .add_message(unspent_msg)
            .add_attributes(attributes))
    } else {
        Ok(Response::new()
            .add_message(send_msg)
            .add_attributes(attributes))
    }
}

#[allow(clippy::too_many_arguments)]
pub fn execute_update_config(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    min_stream_blocks: Option<u64>,
    min_blocks_until_start: Option<u64>,
    stream_creation_denom: Option<String>,
    stream_creation_fee: Option<Uint128>,
    fee_collector: Option<String>,
    accepted_in_denom: Option<String>,
    exit_fee_percent: Option<Decimal>,
    oracle_contract: Option<String>,
) -> Result<Response, ContractError> {
    let mut cfg = CONFIG.load(deps.storage)?;

    if info.sender != cfg.protocol_admin {
        return Err(ContractError::Unauthorized {});
    }

    if let Some(stream_creation_fee) = stream_creation_fee {
        if stream_creation_fee.is_zero() {
            return Err(ContractError::InvalidStreamCreationFee {});
        }
    }
    // exit fee percent can not be equal to or greater than 1, or smaller than 0
    if let Some(exit_fee_percent) = exit_fee_percent {
        if exit_fee_percent >= Decimal::one() || exit_fee_percent < Decimal::zero() {
            return Err(ContractError::InvalidExitFeePercent {});
        }
    }

    cfg.min_stream_blocks = min_stream_blocks.unwrap_or(cfg.min_stream_blocks);
    cfg.min_blocks_until_start_block =
        min_blocks_until_start.unwrap_or(cfg.min_blocks_until_start_block);
    cfg.stream_creation_denom = stream_creation_denom.unwrap_or(cfg.stream_creation_denom);
    cfg.stream_creation_fee = stream_creation_fee.unwrap_or(cfg.stream_creation_fee);
    cfg.accepted_in_denom = accepted_in_denom.unwrap_or(cfg.accepted_in_denom);
    let collector = maybe_addr(deps.api, fee_collector)?.unwrap_or(cfg.fee_collector);
    cfg.fee_collector = collector;
    cfg.exit_fee_percent = exit_fee_percent.unwrap_or(cfg.exit_fee_percent);
    cfg.oracle_contract = maybe_addr(deps.api, oracle_contract)?.unwrap_or(cfg.oracle_contract);

    CONFIG.save(deps.storage, &cfg)?;

    let attributes = vec![
        attr("action", "update_config"),
        attr("min_stream_blocks", cfg.min_stream_blocks.to_string()),
        attr(
            "min_blocks_until_start",
            cfg.min_blocks_until_start_block.to_string(),
        ),
        attr("stream_creation_denom", cfg.stream_creation_denom),
        attr("stream_creation_fee", cfg.stream_creation_fee),
        attr("fee_collector", cfg.fee_collector),
    ];

    Ok(Response::default().add_attributes(attributes))
}

fn check_access(
    info: &MessageInfo,
    position_owner: &Addr,
    position_operator: &Option<Addr>,
) -> Result<(), ContractError> {
    if position_owner.as_ref() != info.sender
        && position_operator
            .as_ref()
            .map_or(true, |o| o != &info.sender)
    {
        return Err(ContractError::Unauthorized {});
    }
    Ok(())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn sudo(deps: DepsMut, env: Env, msg: SudoMsg) -> Result<Response, ContractError> {
    match msg {
        SudoMsg::PauseStream { stream_id } => sudo::sudo_pause_stream(deps, env, stream_id),
        SudoMsg::CancelStream { stream_id } => sudo::sudo_cancel_stream(deps, env, stream_id),
        SudoMsg::ResumeStream { stream_id } => sudo::sudo_resume_stream(deps, env, stream_id),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    let contract_info = get_contract_version(deps.storage)?;
    let storage_contract_name: String = contract_info.contract;
    let storage_version: Version = contract_info.version.parse().map_err(from_semver)?;
    let version: Version = CONTRACT_VERSION.parse().map_err(from_semver)?;

    if storage_contract_name != CONTRACT_NAME {
        return Err(ContractError::CannotMigrate {
            previous_contract: storage_contract_name,
        });
    }
    if storage_version < version {
        set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
        // Code to facilitate state change goes here
    }
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::Stream { stream_id } => to_binary(&query_stream(deps, env, stream_id)?),
        QueryMsg::Position { stream_id, owner } => {
            to_binary(&query_position(deps, env, stream_id, owner)?)
        }
        QueryMsg::ListStreams { start_after, limit } => {
            to_binary(&list_streams(deps, start_after, limit)?)
        }
        QueryMsg::ListPositions {
            stream_id,
            start_after,
            limit,
        } => to_binary(&list_positions(deps, stream_id, start_after, limit)?),
        QueryMsg::AveragePrice { stream_id } => {
            to_binary(&query_average_price(deps, env, stream_id)?)
        }
        QueryMsg::LastStreamedPrice { stream_id } => {
            to_binary(&query_last_streamed_price(deps, env, stream_id)?)
        }
        QueryMsg::Price { denom } => to_binary(&query_oracle(deps, env, denom)?),
    }
}
pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let cfg = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse {
        min_stream_blocks: cfg.min_stream_blocks,
        min_blocks_until_start_block: cfg.min_blocks_until_start_block,
        stream_creation_denom: cfg.stream_creation_denom,
        stream_creation_fee: cfg.stream_creation_fee,
        exit_fee_percent: cfg.exit_fee_percent,
        fee_collector: cfg.fee_collector.to_string(),
        protocol_admin: cfg.protocol_admin.to_string(),
        accepted_in_denom: cfg.accepted_in_denom,
        oracle_contract: cfg.oracle_contract.to_string(),
    })
}

pub fn query_stream(deps: Deps, _env: Env, stream_id: u64) -> StdResult<StreamResponse> {
    let stream = STREAMS.load(deps.storage, stream_id)?;
    let stream = StreamResponse {
        id: stream_id,
        treasury: stream.treasury.to_string(),
        in_denom: stream.in_denom,
        out_denom: stream.out_denom,
        out_supply: stream.out_supply,
        start_block: stream.start_block,
        end_block: stream.end_block,
        spent_in: stream.spent_in,
        dist_index: stream.dist_index,
        out_remaining: stream.out_remaining,
        in_supply: stream.in_supply,
        shares: stream.shares,
        last_updated_block: stream.last_updated_block,
        status: stream.status,
        pause_block: stream.pause_block,
        url: stream.url,
        current_streamed_price: stream.current_streamed_price,
        exit_fee_percent: stream.stream_exit_fee_percent,
        stream_creation_fee: stream.stream_creation_fee,
        stream_creator_address: stream.stream_creator_addr.to_string(),
    };
    Ok(stream)
}

// settings for pagination
const MAX_LIMIT: u32 = 30;
const DEFAULT_LIMIT: u32 = 10;

pub fn list_streams(
    deps: Deps,
    start_after: Option<u64>,
    limit: Option<u32>,
) -> StdResult<StreamsResponse> {
    let start = start_after.map(Bound::exclusive);
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let streams: StdResult<Vec<StreamResponse>> = STREAMS
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|item| {
            let (stream_id, stream) = item?;
            let stream = StreamResponse {
                id: stream_id,
                treasury: stream.treasury.to_string(),
                in_denom: stream.in_denom,
                out_denom: stream.out_denom,
                out_supply: stream.out_supply,
                start_block: stream.start_block,
                end_block: stream.end_block,
                spent_in: stream.spent_in,
                last_updated_block: stream.last_updated_block,
                dist_index: stream.dist_index,
                out_remaining: stream.out_remaining,
                in_supply: stream.in_supply,
                shares: stream.shares,
                status: stream.status,
                pause_block: stream.pause_block,
                url: stream.url,
                current_streamed_price: stream.current_streamed_price,
                exit_fee_percent: stream.stream_exit_fee_percent,
                stream_creation_fee: stream.stream_creation_fee,
                stream_creator_address: stream.stream_creator_addr.to_string(),
            };
            Ok(stream)
        })
        .collect();
    let streams = streams?;
    Ok(StreamsResponse { streams })
}
pub fn query_oracle(deps: Deps, _env: Env, denom: String) -> StdResult<PriceResponse> {
    // This query is for checking if the oracle contract is returning the price for the given denom
    let config = CONFIG.load(deps.storage).unwrap();
    let price_query_msg: QueryMsg = QueryMsg::Price { denom: denom };
    let price: PriceResponse = deps
        .querier
        .query_wasm_smart(config.oracle_contract, &price_query_msg)
        .unwrap();
    Ok(price)
}

pub fn query_position(
    deps: Deps,
    _env: Env,
    stream_id: u64,
    owner: String,
) -> StdResult<PositionResponse> {
    let owner = deps.api.addr_validate(&owner)?;
    let position = POSITIONS.load(deps.storage, (stream_id, &owner))?;
    let res = PositionResponse {
        stream_id,
        owner: owner.to_string(),
        in_balance: position.in_balance,
        purchased: position.purchased,
        index: position.index,
        spent: position.spent,
        shares: position.shares,
        operator: position.operator,
        last_updated_block: position.last_updated_block,
        pending_purchase: position.pending_purchase,
    };
    Ok(res)
}

pub fn list_positions(
    deps: Deps,
    stream_id: u64,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<PositionsResponse> {
    let addr = maybe_addr(deps.api, start_after)?;
    let start = addr.as_ref().map(Bound::exclusive);
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;

    let positions: StdResult<Vec<PositionResponse>> = POSITIONS
        .prefix(stream_id)
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|item| {
            let (owner, position) = item?;
            let position = PositionResponse {
                stream_id,
                owner: owner.to_string(),
                index: position.index,
                last_updated_block: position.last_updated_block,
                purchased: position.purchased,
                pending_purchase: position.pending_purchase,
                spent: position.spent,
                in_balance: position.in_balance,
                shares: position.shares,
                operator: position.operator,
            };
            Ok(position)
        })
        .collect();
    let positions = positions?;
    Ok(PositionsResponse { positions })
}

pub fn query_average_price(
    deps: Deps,
    _env: Env,
    stream_id: u64,
) -> StdResult<AveragePriceResponse> {
    let stream = STREAMS.load(deps.storage, stream_id)?;
    let total_purchased = stream.out_supply - stream.out_remaining;
    let average_price = Decimal::from_ratio(stream.spent_in, total_purchased);
    Ok(AveragePriceResponse { average_price })
}

pub fn query_last_streamed_price(
    deps: Deps,
    _env: Env,
    stream_id: u64,
) -> StdResult<LatestStreamedPriceResponse> {
    let stream = STREAMS.load(deps.storage, stream_id)?;
    Ok(LatestStreamedPriceResponse {
        current_streamed_price: stream.current_streamed_price,
    })
}
