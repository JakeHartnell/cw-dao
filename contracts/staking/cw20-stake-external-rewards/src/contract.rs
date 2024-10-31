use crate::msg::{
    ExecuteMsg, InfoResponse, InstantiateMsg, MigrateMsg, PendingRewardsResponse, QueryMsg,
    ReceiveMsg,
};
use crate::state::{
    Config, RewardConfig, CONFIG, LAST_UPDATE_BLOCK, PENDING_REWARDS, REWARD_CONFIG,
    REWARD_PER_TOKEN, USER_REWARD_PER_TOKEN,
};
use crate::ContractError;
use crate::ContractError::{
    InvalidCw20, InvalidFunds, NoRewardsClaimable, RewardPeriodNotFinished,
};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    from_json, to_json_binary, Addr, BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut, Empty, Env,
    MessageInfo, Response, StdError, StdResult, Uint128, Uint256, WasmMsg,
};
use cw2::{get_contract_version, set_contract_version, ContractVersion};
use cw20::{Cw20ReceiveMsg, Denom};
use dao_hooks::stake::StakeChangedHookMsg;

use cw20::Denom::Cw20;
use std::cmp::min;
use std::convert::TryInto;

const CONTRACT_NAME: &str = "crates.io:cw20-stake-external-rewards";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response<Empty>, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    cw_ownable::initialize_owner(deps.storage, deps.api, msg.owner.as_deref())?;

    let reward_token = match msg.reward_token {
        Denom::Native(denom) => Denom::Native(denom),
        Cw20(addr) => Cw20(deps.api.addr_validate(addr.as_ref())?),
    };

    // Verify contract provided is a staking contract
    let _: cw20_stake::msg::TotalStakedAtHeightResponse = deps.querier.query_wasm_smart(
        &msg.staking_contract,
        &cw20_stake::msg::QueryMsg::TotalStakedAtHeight { height: None },
    )?;

    let config = Config {
        staking_contract: deps.api.addr_validate(&msg.staking_contract)?,
        reward_token,
    };
    CONFIG.save(deps.storage, &config)?;

    if msg.reward_duration == 0 {
        return Err(ContractError::ZeroRewardDuration {});
    }

    let reward_config = RewardConfig {
        period_finish: 0,
        reward_rate: Uint128::zero(),
        reward_duration: msg.reward_duration,
    };
    REWARD_CONFIG.save(deps.storage, &reward_config)?;

    Ok(Response::new()
        .add_attribute("owner", msg.owner.unwrap_or_else(|| "None".to_string()))
        .add_attribute("staking_contract", config.staking_contract)
        .add_attribute(
            "reward_token",
            match config.reward_token {
                Denom::Native(denom) => denom,
                Cw20(addr) => addr.into_string(),
            },
        )
        .add_attribute("reward_rate", reward_config.reward_rate)
        .add_attribute("period_finish", reward_config.period_finish.to_string())
        .add_attribute("reward_duration", reward_config.reward_duration.to_string()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, msg: MigrateMsg) -> Result<Response, ContractError> {
    use cw20_stake_external_rewards_v1 as v1;

    let ContractVersion { version, .. } = get_contract_version(deps.storage)?;
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    match msg {
        MigrateMsg::FromV1 {} => {
            if version == CONTRACT_VERSION {
                // You can not possibly be migrating from v1 to v2 and
                // also not changing your contract version.
                return Err(ContractError::AlreadyMigrated {});
            }
            // From v1 -> v2 we moved `owner` out of config and into
            // the `cw_ownable` package.
            let config = v1::state::CONFIG.load(deps.storage)?;
            cw_ownable::initialize_owner(
                deps.storage,
                deps.api,
                config.owner.map(|a| a.into_string()).as_deref(),
            )?;
            let config = Config {
                staking_contract: config.staking_contract,
                reward_token: match config.reward_token {
                    cw20_013::Denom::Native(n) => Denom::Native(n),
                    cw20_013::Denom::Cw20(a) => Denom::Cw20(a),
                },
            };
            CONFIG.save(deps.storage, &config)?;

            Ok(Response::default())
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response<Empty>, ContractError> {
    match msg {
        ExecuteMsg::StakeChangeHook(msg) => execute_stake_changed(deps, env, info, msg),
        ExecuteMsg::Claim {} => execute_claim(deps, env, info),
        ExecuteMsg::Fund {} => execute_fund_native(deps, env, info),
        ExecuteMsg::Receive(msg) => execute_receive(deps, env, info, msg),
        ExecuteMsg::UpdateRewardDuration { new_duration } => {
            execute_update_reward_duration(deps, env, info, new_duration)
        }
        ExecuteMsg::UpdateOwnership(action) => execute_update_owner(deps, info, env, action),
    }
}

pub fn execute_receive(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    wrapper: Cw20ReceiveMsg,
) -> Result<Response<Empty>, ContractError> {
    let msg: ReceiveMsg = from_json(&wrapper.msg)?;
    let config = CONFIG.load(deps.storage)?;
    let sender = deps.api.addr_validate(&wrapper.sender)?;
    if config.reward_token != Denom::Cw20(info.sender) {
        return Err(InvalidCw20 {});
    };
    match msg {
        ReceiveMsg::Fund {} => execute_fund(deps, env, sender, wrapper.amount),
    }
}

pub fn execute_fund_native(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response<Empty>, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    match config.reward_token {
        Denom::Native(denom) => {
            let amount = cw_utils::must_pay(&info, &denom).map_err(|_| InvalidFunds {})?;
            execute_fund(deps, env, info.sender, amount)
        }
        Cw20(_) => Err(InvalidFunds {}),
    }
}

pub fn execute_fund(
    mut deps: DepsMut,
    env: Env,
    sender: Addr,
    amount: Uint128,
) -> Result<Response<Empty>, ContractError> {
    cw_ownable::assert_owner(deps.storage, &sender)?;

    update_rewards(&mut deps, &env, &sender)?;
    let reward_config = REWARD_CONFIG.load(deps.storage)?;
    if reward_config.period_finish > env.block.height {
        return Err(RewardPeriodNotFinished {});
    }
    let new_reward_config = RewardConfig {
        period_finish: env.block.height + reward_config.reward_duration,
        reward_rate: amount
            .checked_div(Uint128::from(reward_config.reward_duration))
            .map_err(StdError::divide_by_zero)?,
        // As we're not changing the value and changing the value
        // validates that the duration is non-zero we don't need to
        // check here.
        reward_duration: reward_config.reward_duration,
    };

    if new_reward_config.reward_rate == Uint128::zero() {
        return Err(ContractError::RewardRateLessThenOnePerBlock {});
    };

    REWARD_CONFIG.save(deps.storage, &new_reward_config)?;
    LAST_UPDATE_BLOCK.save(deps.storage, &env.block.height)?;

    Ok(Response::new()
        .add_attribute("action", "fund")
        .add_attribute("amount", amount)
        .add_attribute("new_reward_rate", new_reward_config.reward_rate.to_string()))
}

pub fn execute_stake_changed(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: StakeChangedHookMsg,
) -> Result<Response<Empty>, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.staking_contract {
        return Err(ContractError::InvalidHookSender {});
    };
    match msg {
        StakeChangedHookMsg::Stake { addr, .. } => execute_stake(deps, env, addr),
        StakeChangedHookMsg::Unstake { addr, .. } => execute_unstake(deps, env, addr),
    }
}

pub fn execute_stake(
    mut deps: DepsMut,
    env: Env,
    addr: Addr,
) -> Result<Response<Empty>, ContractError> {
    update_rewards(&mut deps, &env, &addr)?;
    Ok(Response::new().add_attribute("action", "stake"))
}

pub fn execute_unstake(
    mut deps: DepsMut,
    env: Env,
    addr: Addr,
) -> Result<Response<Empty>, ContractError> {
    update_rewards(&mut deps, &env, &addr)?;
    Ok(Response::new().add_attribute("action", "unstake"))
}

pub fn execute_claim(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response<Empty>, ContractError> {
    update_rewards(&mut deps, &env, &info.sender)?;
    let rewards = PENDING_REWARDS
        .load(deps.storage, info.sender.clone())
        .map_err(|_| NoRewardsClaimable {})?;
    if rewards == Uint128::zero() {
        return Err(ContractError::NoRewardsClaimable {});
    }
    PENDING_REWARDS.save(deps.storage, info.sender.clone(), &Uint128::zero())?;
    let config = CONFIG.load(deps.storage)?;
    let transfer_msg = get_transfer_msg(info.sender, rewards, config.reward_token)?;
    Ok(Response::new()
        .add_message(transfer_msg)
        .add_attribute("action", "claim")
        .add_attribute("amount", rewards))
}

pub fn execute_update_owner(
    deps: DepsMut,
    info: MessageInfo,
    env: Env,
    action: cw_ownable::Action,
) -> Result<Response, ContractError> {
    let ownership = cw_ownable::update_ownership(deps, &env.block, &info.sender, action)?;
    Ok(Response::default().add_attributes(ownership.into_attributes()))
}

pub fn get_transfer_msg(recipient: Addr, amount: Uint128, denom: Denom) -> StdResult<CosmosMsg> {
    match denom {
        Denom::Native(denom) => Ok(BankMsg::Send {
            to_address: recipient.into_string(),
            amount: vec![Coin { denom, amount }],
        }
        .into()),
        Denom::Cw20(addr) => {
            let cw20_msg = to_json_binary(&cw20::Cw20ExecuteMsg::Transfer {
                recipient: recipient.into_string(),
                amount,
            })?;
            Ok(WasmMsg::Execute {
                contract_addr: addr.into_string(),
                msg: cw20_msg,
                funds: vec![],
            }
            .into())
        }
    }
}

pub fn update_rewards(deps: &mut DepsMut, env: &Env, addr: &Addr) -> StdResult<()> {
    let config = CONFIG.load(deps.storage)?;
    let reward_per_token = get_reward_per_token(deps.as_ref(), env, &config.staking_contract)?;
    REWARD_PER_TOKEN.save(deps.storage, &reward_per_token)?;

    let earned_rewards = get_rewards_earned(
        deps.as_ref(),
        env,
        addr,
        reward_per_token,
        &config.staking_contract,
    )?;
    PENDING_REWARDS.update::<_, StdError>(deps.storage, addr.clone(), |r| {
        Ok(r.unwrap_or_default() + earned_rewards)
    })?;

    USER_REWARD_PER_TOKEN.save(deps.storage, addr.clone(), &reward_per_token)?;
    let last_time_reward_applicable = get_last_time_reward_applicable(deps.as_ref(), env)?;
    LAST_UPDATE_BLOCK.save(deps.storage, &last_time_reward_applicable)?;
    Ok(())
}

pub fn get_reward_per_token(deps: Deps, env: &Env, staking_contract: &Addr) -> StdResult<Uint256> {
    let reward_config = REWARD_CONFIG.load(deps.storage)?;
    let total_staked = get_total_staked(deps, staking_contract)?;
    let last_time_reward_applicable = get_last_time_reward_applicable(deps, env)?;
    let last_update_block = LAST_UPDATE_BLOCK.load(deps.storage).unwrap_or_default();
    let prev_reward_per_token = REWARD_PER_TOKEN.load(deps.storage).unwrap_or_default();
    let additional_reward_per_token = if total_staked == Uint128::zero() {
        Uint256::zero()
    } else {
        // It is impossible for this to overflow as total rewards can never exceed max value of
        // Uint128 as total tokens in existence cannot exceed Uint128
        let numerator = reward_config
            .reward_rate
            .full_mul(Uint128::from(
                last_time_reward_applicable - last_update_block,
            ))
            .checked_mul(scale_factor())?;
        let denominator = Uint256::from(total_staked);
        numerator.checked_div(denominator)?
    };

    Ok(prev_reward_per_token + additional_reward_per_token)
}

pub fn get_rewards_earned(
    deps: Deps,
    _env: &Env,
    addr: &Addr,
    reward_per_token: Uint256,
    staking_contract: &Addr,
) -> StdResult<Uint128> {
    let _config = CONFIG.load(deps.storage)?;
    let staked_balance = Uint256::from(get_staked_balance(deps, staking_contract, addr)?);
    let user_reward_per_token = USER_REWARD_PER_TOKEN
        .load(deps.storage, addr.clone())
        .unwrap_or_default();
    let reward_factor = reward_per_token.checked_sub(user_reward_per_token)?;
    Ok(staked_balance
        .checked_mul(reward_factor)?
        .checked_div(scale_factor())?
        .try_into()?)
}

fn get_last_time_reward_applicable(deps: Deps, env: &Env) -> StdResult<u64> {
    let reward_config = REWARD_CONFIG.load(deps.storage)?;
    Ok(min(env.block.height, reward_config.period_finish))
}

fn get_total_staked(deps: Deps, contract_addr: &Addr) -> StdResult<Uint128> {
    let msg = cw20_stake::msg::QueryMsg::TotalStakedAtHeight { height: None };
    let resp: cw20_stake::msg::TotalStakedAtHeightResponse =
        deps.querier.query_wasm_smart(contract_addr, &msg)?;
    Ok(resp.total)
}

fn get_staked_balance(deps: Deps, contract_addr: &Addr, addr: &Addr) -> StdResult<Uint128> {
    let msg = cw20_stake::msg::QueryMsg::StakedBalanceAtHeight {
        address: addr.into(),
        height: None,
    };
    let resp: cw20_stake::msg::StakedBalanceAtHeightResponse =
        deps.querier.query_wasm_smart(contract_addr, &msg)?;
    Ok(resp.balance)
}

pub fn execute_update_reward_duration(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    new_duration: u64,
) -> Result<Response<Empty>, ContractError> {
    cw_ownable::assert_owner(deps.storage, &info.sender)?;

    let mut reward_config = REWARD_CONFIG.load(deps.storage)?;
    if reward_config.period_finish > env.block.height {
        return Err(ContractError::RewardPeriodNotFinished {});
    };

    if new_duration == 0 {
        return Err(ContractError::ZeroRewardDuration {});
    }

    let old_duration = reward_config.reward_duration;
    reward_config.reward_duration = new_duration;
    REWARD_CONFIG.save(deps.storage, &reward_config)?;

    Ok(Response::new()
        .add_attribute("action", "update_reward_duration")
        .add_attribute("new_duration", new_duration.to_string())
        .add_attribute("old_duration", old_duration.to_string()))
}

fn scale_factor() -> Uint256 {
    Uint256::from(10u8).pow(39)
}
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Info {} => Ok(to_json_binary(&query_info(deps, env)?)?),
        QueryMsg::GetPendingRewards { address } => {
            Ok(to_json_binary(&query_pending_rewards(deps, env, address)?)?)
        }
        QueryMsg::Ownership {} => to_json_binary(&cw_ownable::get_ownership(deps.storage)?),
    }
}

pub fn query_info(deps: Deps, _env: Env) -> StdResult<InfoResponse> {
    let config = CONFIG.load(deps.storage)?;
    let reward = REWARD_CONFIG.load(deps.storage)?;
    Ok(InfoResponse { config, reward })
}

pub fn query_pending_rewards(
    deps: Deps,
    env: Env,
    addr: String,
) -> StdResult<PendingRewardsResponse> {
    let addr = deps.api.addr_validate(&addr)?;
    let config = CONFIG.load(deps.storage)?;
    let reward_per_token = get_reward_per_token(deps, &env, &config.staking_contract)?;
    let earned_rewards = get_rewards_earned(
        deps,
        &env,
        &addr,
        reward_per_token,
        &config.staking_contract,
    )?;

    let existing_rewards = PENDING_REWARDS
        .load(deps.storage, addr.clone())
        .unwrap_or_default();
    let pending_rewards = earned_rewards + existing_rewards;
    Ok(PendingRewardsResponse {
        address: addr.to_string(),
        pending_rewards,
        denom: config.reward_token,
        last_update_block: LAST_UPDATE_BLOCK.load(deps.storage).unwrap_or_default(),
    })
}
