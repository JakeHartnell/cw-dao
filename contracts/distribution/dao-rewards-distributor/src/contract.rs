#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    ensure, from_json, to_json_binary, Addr, Binary, Deps, DepsMut, Env, MessageInfo, Order,
    Response, StdError, StdResult, Uint128, Uint256,
};
use cw2::{get_contract_version, set_contract_version};
use cw20::{Cw20ReceiveMsg, Denom, UncheckedDenom};
use cw_storage_plus::Bound;
use cw_utils::{must_pay, nonpayable, Duration, Expiration};
use dao_interface::voting::InfoResponse;
use semver::Version;

use std::ops::Add;

use crate::helpers::{get_transfer_msg, validate_voting_power_contract};
use crate::hooks::{
    execute_membership_changed, execute_nft_stake_changed, execute_stake_changed,
    subscribe_distribution_to_hook, unsubscribe_distribution_from_hook,
};
use crate::msg::{
    CreateMsg, DistributionPendingRewards, DistributionsResponse, ExecuteMsg, FundMsg,
    InstantiateMsg, MigrateMsg, PendingRewardsResponse, QueryMsg, ReceiveCw20Msg,
};
use crate::rewards::{
    get_accrued_rewards_not_yet_accounted_for, get_active_total_earned_puvp, update_rewards,
};
use crate::state::{DistributionState, EmissionRate, Epoch, COUNT, DISTRIBUTIONS, USER_REWARDS};
use crate::ContractError;

pub(crate) const CONTRACT_NAME: &str = env!("CARGO_PKG_NAME");
pub(crate) const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub const DEFAULT_LIMIT: u32 = 10;
pub const MAX_LIMIT: u32 = 50;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    // Intialize the contract owner, defaulting to instantiator.
    let owner = deps
        .api
        .addr_validate(&msg.owner.unwrap_or_else(|| info.sender.to_string()))?;
    cw_ownable::initialize_owner(deps.storage, deps.api, Some(owner.as_str()))?;

    // initialize count
    COUNT.save(deps.storage, &0)?;

    Ok(Response::new().add_attribute("owner", owner))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::StakeChangeHook(msg) => execute_stake_changed(deps, env, info, msg),
        ExecuteMsg::NftStakeChangeHook(msg) => execute_nft_stake_changed(deps, env, info, msg),
        ExecuteMsg::MemberChangedHook(msg) => execute_membership_changed(deps, env, info, msg),
        ExecuteMsg::UpdateOwnership(action) => execute_update_owner(deps, info, env, action),
        ExecuteMsg::Receive(msg) => execute_receive_cw20(deps, env, info, msg),
        ExecuteMsg::Create(create_msg) => execute_create(deps, env, info, create_msg),
        ExecuteMsg::Update {
            id,
            emission_rate,
            vp_contract,
            hook_caller,
            open_funding,
            withdraw_destination,
        } => execute_update(
            deps,
            env,
            info,
            id,
            emission_rate,
            vp_contract,
            hook_caller,
            open_funding,
            withdraw_destination,
        ),
        ExecuteMsg::Fund(FundMsg { id }) => execute_fund_native(deps, env, info, id),
        ExecuteMsg::FundLatest {} => execute_fund_latest_native(deps, env, info),
        ExecuteMsg::Claim { id } => execute_claim(deps, env, info, id),
        ExecuteMsg::Withdraw { id } => execute_withdraw(deps, info, env, id),
        ExecuteMsg::UnsafeForceWithdraw { amount, denom } => {
            execute_unsafe_force_withdraw(deps, info, amount, denom)
        }
    }
}

fn execute_receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    wrapper: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    nonpayable(&info)?;

    // verify msg
    let msg: ReceiveCw20Msg = from_json(&wrapper.msg)?;

    match msg {
        ReceiveCw20Msg::Fund(FundMsg { id }) => {
            let distribution = DISTRIBUTIONS
                .load(deps.storage, id)
                .map_err(|_| ContractError::DistributionNotFound { id })?;

            match &distribution.denom {
                Denom::Native(_) => return Err(ContractError::InvalidFunds {}),
                Denom::Cw20(addr) => {
                    // ensure funding is coming from the cw20 we are currently
                    // distributing
                    if addr != info.sender {
                        return Err(ContractError::InvalidCw20 {});
                    }
                }
            };

            let sender = deps.api.addr_validate(&wrapper.sender)?;

            execute_fund(deps, env, sender, distribution, wrapper.amount)
        }
        ReceiveCw20Msg::FundLatest {} => {
            let id = COUNT.load(deps.storage)?;
            let distribution = DISTRIBUTIONS
                .load(deps.storage, id)
                .map_err(|_| ContractError::DistributionNotFound { id })?;

            match &distribution.denom {
                Denom::Native(_) => return Err(ContractError::InvalidFunds {}),
                Denom::Cw20(addr) => {
                    // ensure funding is coming from the cw20 we are currently
                    // distributing
                    if addr != info.sender {
                        return Err(ContractError::InvalidCw20 {});
                    }
                }
            };

            let sender = deps.api.addr_validate(&wrapper.sender)?;

            execute_fund(deps, env, sender, distribution, wrapper.amount)
        }
    }
}

/// creates a new rewards distribution. only the owner can do this. if funds
/// provided when creating a native token distribution, will start distributing
/// rewards immediately.
fn execute_create(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: CreateMsg,
) -> Result<Response, ContractError> {
    // only the owner can create a new distribution
    cw_ownable::assert_owner(deps.storage, &info.sender)?;

    // update count and use as the new distribution's ID
    let id = COUNT.update(deps.storage, |count| -> StdResult<u64> { Ok(count + 1) })?;

    let checked_denom = msg.denom.into_checked(deps.as_ref())?;
    let hook_caller = deps.api.addr_validate(&msg.hook_caller)?;
    let vp_contract = validate_voting_power_contract(&deps, msg.vp_contract)?;

    let withdraw_destination = match msg.withdraw_destination {
        // if withdraw destination is specified, we validate it
        Some(addr) => deps.api.addr_validate(&addr)?,
        // otherwise default to the owner
        None => info.sender.clone(),
    };

    msg.emission_rate.validate()?;

    let open_funding = msg.open_funding.unwrap_or(true);

    // Initialize the distribution state
    let distribution = DistributionState {
        id,
        denom: checked_denom,
        active_epoch: Epoch {
            started_at: Expiration::Never {},
            ends_at: Expiration::Never {},
            emission_rate: msg.emission_rate,
            total_earned_puvp: Uint256::zero(),
            last_updated_total_earned_puvp: Expiration::Never {},
        },
        vp_contract,
        hook_caller: hook_caller.clone(),
        funded_amount: Uint128::zero(),
        open_funding,
        withdraw_destination,
        historical_earned_puvp: Uint256::zero(),
    };

    // store the new distribution state, erroring if it already exists. this
    // should never happen, but just in case.
    DISTRIBUTIONS.update(deps.storage, id, |existing| match existing {
        Some(_) => Err(ContractError::UnexpectedDuplicateDistributionId { id }),
        None => Ok(distribution.clone()),
    })?;

    // update the registered hooks to include the new distribution
    subscribe_distribution_to_hook(deps.storage, id, hook_caller.clone())?;

    let mut response = Response::new()
        .add_attribute("action", "create")
        .add_attribute("id", id.to_string())
        .add_attribute("denom", distribution.get_denom_string());

    // if native funds provided, ensure they are for this denom. if other native
    // funds present, return error. if no funds, do nothing and leave registered
    // denom with no funding, to be funded later.
    if !info.funds.is_empty() {
        match &distribution.denom {
            Denom::Native(denom) => {
                // ensures there is exactly 1 coin passed that matches the denom
                let initial_funds = must_pay(&info, denom)?;

                execute_fund(deps, env, info.sender, distribution, initial_funds)?;

                response = response.add_attribute("amount_funded", initial_funds);
            }
            Denom::Cw20(_) => return Err(ContractError::NoFundsOnCw20Create {}),
        }
    }

    Ok(response)
}

/// updates the config for a distribution
#[allow(clippy::too_many_arguments)]
fn execute_update(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    id: u64,
    emission_rate: Option<EmissionRate>,
    vp_contract: Option<String>,
    hook_caller: Option<String>,
    open_funding: Option<bool>,
    withdraw_destination: Option<String>,
) -> Result<Response, ContractError> {
    nonpayable(&info)?;

    // only the owner can update a distribution
    cw_ownable::assert_owner(deps.storage, &info.sender)?;

    let mut distribution = DISTRIBUTIONS
        .load(deps.storage, id)
        .map_err(|_| ContractError::DistributionNotFound { id })?;

    if let Some(emission_rate) = emission_rate {
        emission_rate.validate()?;

        // transition the epoch to the new emission rate
        distribution.transition_epoch(deps.as_ref(), emission_rate, &env.block)?;
    }

    if let Some(vp_contract) = vp_contract {
        distribution.vp_contract = validate_voting_power_contract(&deps, vp_contract)?;
    }

    if let Some(hook_caller) = hook_caller {
        // remove existing from registered hooks
        unsubscribe_distribution_from_hook(deps.storage, id, distribution.hook_caller)?;

        distribution.hook_caller = deps.api.addr_validate(&hook_caller)?;

        // add new to registered hooks
        subscribe_distribution_to_hook(deps.storage, id, distribution.hook_caller.clone())?;
    }

    if let Some(open_funding) = open_funding {
        distribution.open_funding = open_funding;
    }

    if let Some(withdraw_destination) = withdraw_destination {
        distribution.withdraw_destination = deps.api.addr_validate(&withdraw_destination)?;
    }

    DISTRIBUTIONS.save(deps.storage, id, &distribution)?;

    Ok(Response::new()
        .add_attribute("action", "update")
        .add_attribute("id", id.to_string())
        .add_attribute("denom", distribution.get_denom_string()))
}

fn execute_fund_latest_native(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let id = COUNT.load(deps.storage)?;
    execute_fund_native(deps, env, info, id)
}

fn execute_fund_native(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    id: u64,
) -> Result<Response, ContractError> {
    let distribution = DISTRIBUTIONS
        .load(deps.storage, id)
        .map_err(|_| ContractError::DistributionNotFound { id })?;

    let amount = match &distribution.denom {
        Denom::Native(denom) => {
            must_pay(&info, denom).map_err(|_| ContractError::InvalidFunds {})?
        }
        Denom::Cw20(_) => return Err(ContractError::InvalidFunds {}),
    };

    execute_fund(deps, env, info.sender, distribution, amount)
}

fn execute_fund(
    deps: DepsMut,
    env: Env,
    sender: Addr,
    distribution: DistributionState,
    amount: Uint128,
) -> Result<Response, ContractError> {
    // only the owner can fund if open_funding is disabled
    if !distribution.open_funding {
        cw_ownable::assert_owner(deps.storage, &sender)?;
    }

    match distribution.active_epoch.emission_rate {
        EmissionRate::Paused {} => execute_fund_paused(deps, distribution, amount),
        EmissionRate::Immediate {} => execute_fund_immediate(deps, env, distribution, amount),
        EmissionRate::Linear { .. } => execute_fund_linear(deps, env, distribution, amount),
    }
}

/// funding a paused distribution simply increases the funded amount.
fn execute_fund_paused(
    deps: DepsMut,
    mut distribution: DistributionState,
    amount: Uint128,
) -> Result<Response, ContractError> {
    distribution.funded_amount += amount;

    DISTRIBUTIONS.save(deps.storage, distribution.id, &distribution)?;

    Ok(Response::new()
        .add_attribute("action", "fund")
        .add_attribute("id", distribution.id.to_string())
        .add_attribute("denom", distribution.get_denom_string())
        .add_attribute("amount_funded", amount))
}

/// funding an immediate distribution instantly distributes the new amount.
fn execute_fund_immediate(
    deps: DepsMut,
    env: Env,
    mut distribution: DistributionState,
    amount: Uint128,
) -> Result<Response, ContractError> {
    distribution.funded_amount += amount;

    // for immediate distribution, update total_earned_puvp instantly since we
    // need to know the change in funded_amount to calculate the new
    // total_earned_puvp.
    distribution.update_immediate_emission_total_earned_puvp(deps.as_ref(), &env.block, amount)?;

    DISTRIBUTIONS.save(deps.storage, distribution.id, &distribution)?;

    Ok(Response::new()
        .add_attribute("action", "fund")
        .add_attribute("id", distribution.id.to_string())
        .add_attribute("denom", distribution.get_denom_string())
        .add_attribute("amount_funded", amount))
}

/// funding a linear distribution requires some complex logic based on whether
/// or not the distribution is continuous and whether or not it's expired.
///
/// expired continuous distributions experience backfill with the new funds,
/// whereas expired discontinuous distributions begin anew (and all past rewards
/// must be taken into account before restarting distribution).
fn execute_fund_linear(
    deps: DepsMut,
    env: Env,
    mut distribution: DistributionState,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let continuous =
        if let EmissionRate::Linear { continuous, .. } = distribution.active_epoch.emission_rate {
            continuous
        } else {
            false
        };
    let previously_funded = !distribution.funded_amount.is_zero();
    let was_expired = distribution.active_epoch.ends_at.is_expired(&env.block);
    let discontinuous_expired = !continuous && was_expired;

    // restart the distribution from the current block if it hasn't yet started
    // (i.e. never been funded) OR if it's both discontinuous and expired (i.e.
    // all past funds should have been distributed and we're effectively
    // beginning a new distribution with new funds). this ensures the new funds
    // start being distributed from now instead of from the past.
    //
    // if already funded and continuous or not expired (else block), just add
    // the new funds and leave start date the same, backfilling rewards.
    if !previously_funded || discontinuous_expired {
        // if funding an expired discontinuous distribution that's previously
        // been funded, ensure all rewards are taken into account before
        // restarting, in case users haven't claimed yet, by adding the final
        // total rewards earned to the historical value.
        if discontinuous_expired && previously_funded {
            let final_total_earned_puvp =
                get_active_total_earned_puvp(deps.as_ref(), &env.block, &distribution)?;
            distribution.historical_earned_puvp = distribution
                .historical_earned_puvp
                .checked_add(final_total_earned_puvp)
                .map_err(|err| ContractError::DistributionHistoryTooLarge {
                    err: err.to_string(),
                })?;
            // last updated block is reset to the new start block below
        }

        // reset all starting fields since a new distribution is starting
        distribution.funded_amount = amount;
        distribution.active_epoch.started_at = match distribution.active_epoch.emission_rate {
            EmissionRate::Paused {} => Expiration::Never {},
            EmissionRate::Immediate {} => Expiration::Never {},
            EmissionRate::Linear { duration, .. } => match duration {
                Duration::Height(_) => Expiration::AtHeight(env.block.height),
                Duration::Time(_) => Expiration::AtTime(env.block.time),
            },
        };
        distribution.active_epoch.total_earned_puvp = Uint256::zero();
        distribution.active_epoch.last_updated_total_earned_puvp =
            distribution.active_epoch.started_at;
    } else {
        distribution.funded_amount += amount;
    }

    // update the end block based on the new funds and potentially updated start
    let new_funded_duration = distribution
        .active_epoch
        .emission_rate
        .get_funded_period_duration(distribution.funded_amount)?;
    distribution.active_epoch.ends_at = match new_funded_duration {
        Some(duration) => distribution.active_epoch.started_at.add(duration)?,
        None => Expiration::Never {},
    };

    // if continuous, funds existed in the past, and the distribution was
    // expired, some rewards may not have been distributed due to lack of
    // sufficient funding. ensure the total rewards earned puvp is up to date
    // based on the original start block and the newly updated end block.
    if continuous && previously_funded && was_expired {
        distribution.active_epoch.total_earned_puvp =
            get_active_total_earned_puvp(deps.as_ref(), &env.block, &distribution)?;
        distribution.active_epoch.bump_last_updated(&env.block);
    }

    DISTRIBUTIONS.save(deps.storage, distribution.id, &distribution)?;

    Ok(Response::new()
        .add_attribute("action", "fund")
        .add_attribute("id", distribution.id.to_string())
        .add_attribute("denom", distribution.get_denom_string())
        .add_attribute("amount_funded", amount))
}

fn execute_claim(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    id: u64,
) -> Result<Response, ContractError> {
    nonpayable(&info)?;

    // update the distribution for the sender. this updates the distribution
    // state and the user reward state.
    update_rewards(&mut deps, &env, &info.sender, id)?;

    // load the updated states. previous `update_rewards` call ensures that
    // these states exist.
    let distribution = DISTRIBUTIONS.load(deps.storage, id)?;
    let mut user_reward_state = USER_REWARDS.load(deps.storage, info.sender.clone())?;

    // updating the map returns the previous value if it existed. we set the
    // value to zero and get the amount of pending rewards until this point.
    let claim_amount = user_reward_state
        .pending_rewards
        .insert(id, Uint128::zero())
        .unwrap_or_default();

    // if there are no rewards to claim, error out
    if claim_amount.is_zero() {
        return Err(ContractError::NoRewardsClaimable {});
    }

    // otherwise reflect the updated user reward state and transfer out the
    // claimed rewards
    USER_REWARDS.save(deps.storage, info.sender.clone(), &user_reward_state)?;

    let denom_str = distribution.get_denom_string();

    Ok(Response::new()
        .add_message(get_transfer_msg(
            info.sender.clone(),
            claim_amount,
            distribution.denom,
        )?)
        .add_attribute("action", "claim")
        .add_attribute("id", id.to_string())
        .add_attribute("denom", denom_str)
        .add_attribute("amount_claimed", claim_amount))
}

/// withdraws the undistributed rewards for a distribution. members can claim
/// whatever they earned until this point. this is effectively an inverse to
/// fund and does not affect any already-distributed rewards. can only be called
/// by the admin and only during the distribution period. updates the period
/// finish expiration to the current block.
fn execute_withdraw(
    deps: DepsMut,
    info: MessageInfo,
    env: Env,
    id: u64,
) -> Result<Response, ContractError> {
    nonpayable(&info)?;

    // only the owner can initiate a withdraw
    cw_ownable::assert_owner(deps.storage, &info.sender)?;

    let mut distribution = DISTRIBUTIONS
        .load(deps.storage, id)
        .map_err(|_| ContractError::DistributionNotFound { id })?;

    // withdraw is only possible during the distribution period
    ensure!(
        !distribution.active_epoch.ends_at.is_expired(&env.block),
        ContractError::RewardsAlreadyDistributed {}
    );

    // withdraw ends the epoch early
    distribution.active_epoch.ends_at = match distribution.active_epoch.started_at {
        Expiration::Never {} => Expiration::Never {},
        Expiration::AtHeight(_) => Expiration::AtHeight(env.block.height),
        Expiration::AtTime(_) => Expiration::AtTime(env.block.time),
    };

    // get total rewards distributed based on newly updated ends_at
    let rewards_distributed = distribution.get_total_rewards()?;

    let clawback_amount = distribution.funded_amount - rewards_distributed;

    // remove withdrawn funds from amount funded since they are no longer funded
    distribution.funded_amount = rewards_distributed;

    let clawback_msg = get_transfer_msg(
        distribution.withdraw_destination.clone(),
        clawback_amount,
        distribution.denom.clone(),
    )?;

    DISTRIBUTIONS.save(deps.storage, id, &distribution)?;

    Ok(Response::new()
        .add_attribute("action", "withdraw")
        .add_attribute("id", id.to_string())
        .add_attribute("denom", distribution.get_denom_string())
        .add_attribute("amount_withdrawn", clawback_amount)
        .add_attribute("amount_distributed", rewards_distributed)
        .add_message(clawback_msg))
}

fn execute_update_owner(
    deps: DepsMut,
    info: MessageInfo,
    env: Env,
    action: cw_ownable::Action,
) -> Result<Response, ContractError> {
    nonpayable(&info)?;

    // Update the current contract owner. Note, this is a two step process, the
    // new owner must accept this ownership transfer. First the owner specifies
    // the new owner, then the new owner must accept.
    let ownership = cw_ownable::update_ownership(deps, &env.block, &info.sender, action)?;
    Ok(Response::new().add_attributes(ownership.into_attributes()))
}

fn execute_unsafe_force_withdraw(
    deps: DepsMut,
    info: MessageInfo,
    amount: Uint128,
    denom: UncheckedDenom,
) -> Result<Response, ContractError> {
    nonpayable(&info)?;

    // only the owner can initiate a force withdraw
    cw_ownable::assert_owner(deps.storage, &info.sender)?;

    let checked_denom = denom.into_checked(deps.as_ref())?;

    let denom_str = match &checked_denom {
        Denom::Native(denom) => denom.to_string(),
        Denom::Cw20(address) => address.to_string(),
    };

    let send = get_transfer_msg(info.sender, amount, checked_denom)?;

    Ok(Response::new()
        .add_message(send)
        .add_attribute("action", "unsafe_force_withdraw")
        .add_attribute("amount", amount.to_string())
        .add_attribute("denom", denom_str))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Info {} => Ok(to_json_binary(&query_info(deps)?)?),
        QueryMsg::Ownership {} => to_json_binary(&cw_ownable::get_ownership(deps.storage)?),
        QueryMsg::PendingRewards {
            address,
            start_after,
            limit,
        } => Ok(to_json_binary(&query_pending_rewards(
            deps,
            env,
            address,
            start_after,
            limit,
        )?)?),
        QueryMsg::UndistributedRewards { id } => Ok(to_json_binary(
            &query_undistributed_rewards(deps, env, id)
                .map_err(|e| StdError::generic_err(e.to_string()))?,
        )?),
        QueryMsg::Distribution { id } => Ok(to_json_binary(
            &query_distribution(deps, id).map_err(|e| StdError::generic_err(e.to_string()))?,
        )?),
        QueryMsg::Distributions { start_after, limit } => Ok(to_json_binary(
            &query_distributions(deps, start_after, limit)?,
        )?),
    }
}

fn query_info(deps: Deps) -> StdResult<InfoResponse> {
    let info = get_contract_version(deps.storage)?;
    Ok(InfoResponse { info })
}

/// returns the pending rewards for a given address that are ready to be
/// claimed.
fn query_pending_rewards(
    deps: Deps,
    env: Env,
    addr: String,
    start_after: Option<u64>,
    limit: Option<u32>,
) -> StdResult<PendingRewardsResponse> {
    let addr = deps.api.addr_validate(&addr)?;

    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start = start_after.map(Bound::<u64>::exclusive);

    // user may not have interacted with the contract before this query so we
    // potentially return the default user reward state
    let user_reward_state = USER_REWARDS
        .load(deps.storage, addr.clone())
        .unwrap_or_default();

    let distributions = DISTRIBUTIONS
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .collect::<StdResult<Vec<_>>>()?;

    let mut pending_rewards: Vec<DistributionPendingRewards> = vec![];

    // iterate over all distributions and calculate pending rewards for the user
    for (id, distribution) in distributions {
        // first we get the active epoch earned puvp value
        let active_total_earned_puvp =
            get_active_total_earned_puvp(deps, &env.block, &distribution)
                .map_err(|e| StdError::generic_err(e.to_string()))?;

        // then we add that to the historical rewards earned puvp
        let total_earned_puvp =
            active_total_earned_puvp.checked_add(distribution.historical_earned_puvp)?;

        let existing_amount = user_reward_state
            .pending_rewards
            .get(&id)
            .cloned()
            .unwrap_or_default();

        let unaccounted_for_rewards = get_accrued_rewards_not_yet_accounted_for(
            deps,
            &env,
            &addr,
            total_earned_puvp,
            &distribution,
            &user_reward_state,
        )?;

        pending_rewards.push(DistributionPendingRewards {
            id,
            denom: distribution.denom,
            pending_rewards: unaccounted_for_rewards + existing_amount,
        });
    }

    Ok(PendingRewardsResponse { pending_rewards })
}

fn query_undistributed_rewards(deps: Deps, env: Env, id: u64) -> Result<Uint128, ContractError> {
    let distribution = query_distribution(deps, id)?;
    let undistributed_rewards = distribution.get_undistributed_rewards(&env.block)?;
    Ok(undistributed_rewards)
}

fn query_distribution(deps: Deps, id: u64) -> Result<DistributionState, ContractError> {
    let state = DISTRIBUTIONS
        .load(deps.storage, id)
        .map_err(|_| ContractError::DistributionNotFound { id })?;
    Ok(state)
}

fn query_distributions(
    deps: Deps,
    start_after: Option<u64>,
    limit: Option<u32>,
) -> StdResult<DistributionsResponse> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start = start_after.map(Bound::<u64>::exclusive);

    let distributions = DISTRIBUTIONS
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|item| item.map(|(_, v)| v))
        .collect::<StdResult<Vec<_>>>()?;

    Ok(DistributionsResponse { distributions })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    let contract_version = get_contract_version(deps.storage)?;

    if contract_version.contract != CONTRACT_NAME {
        return Err(ContractError::MigrationErrorIncorrectContract {
            expected: CONTRACT_NAME.to_string(),
            actual: contract_version.contract,
        });
    }

    let new_version: Version = CONTRACT_VERSION.parse()?;
    let current_version: Version = contract_version.version.parse()?;

    // only allow upgrades
    if new_version <= current_version {
        return Err(ContractError::MigrationErrorInvalidVersionNotNewer {
            new: new_version.to_string(),
            current: current_version.to_string(),
        });
    }

    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    Ok(Response::default())
}
