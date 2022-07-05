#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    coins, to_binary, BankMsg, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response,
    StdResult, Uint128,
};
use cw2::set_contract_version;
use cw_controllers::ClaimsResponse;
use cw_core_interface::voting::{TotalPowerAtHeightResponse, VotingPowerAtHeightResponse};
use cw_utils::{must_pay, Duration};

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, GetConfigResponse, InstantiateMsg, MigrateMsg, QueryMsg};
use crate::state::{Config, CLAIMS, CONFIG, DAO, MAX_CLAIMS, STAKED_BALANCES, STAKED_TOTAL};

const CONTRACT_NAME: &str = "crates.io:cw-native-staked-balance-voting";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    let owner = match msg.owner {
        Some(owner) => Some(deps.api.addr_validate(&owner)?),
        None => None,
    };

    let manager = match msg.manager {
        Some(manager) => Some(deps.api.addr_validate(&manager)?),
        None => None,
    };

    let config = Config {
        owner,
        manager,
        denom: msg.denom,
        unstaking_duration: msg.unstaking_duration,
    };

    CONFIG.save(deps.storage, &config)?;
    DAO.save(deps.storage, &info.sender)?;

    Ok(Response::new().add_attribute("action", "instantiate"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Stake {} => execute_stake(deps, env, info),
        ExecuteMsg::Unstake { amount } => execute_unstake(deps, env, info, amount),
        ExecuteMsg::UpdateConfig {
            owner,
            manager,
            duration,
        } => execute_update_config(deps, info, owner, manager, duration),
        ExecuteMsg::Claim {} => execute_claim(deps, env, info),
    }
}

pub fn execute_stake(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let amount = must_pay(&info, &config.denom).map_err(ContractError::PaymentError)?;

    STAKED_BALANCES.update(
        deps.storage,
        &info.sender,
        env.block.height,
        |balance| -> StdResult<Uint128> { Ok(balance.unwrap_or_default().checked_add(amount)?) },
    )?;
    STAKED_TOTAL.update(
        deps.storage,
        env.block.height,
        |total| -> StdResult<Uint128> { Ok(total.unwrap_or_default().checked_add(amount)?) },
    )?;

    Ok(Response::new()
        .add_attribute("action", "stake")
        .add_attribute("amount", amount.to_string())
        .add_attribute("from", info.sender))
}

pub fn execute_unstake(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    STAKED_BALANCES.update(
        deps.storage,
        &info.sender,
        env.block.height,
        |balance| -> StdResult<Uint128> { Ok(balance.unwrap_or_default().checked_sub(amount)?) },
    )?;
    STAKED_TOTAL.update(
        deps.storage,
        env.block.height,
        |total| -> StdResult<Uint128> { Ok(total.unwrap_or_default().checked_sub(amount)?) },
    )?;

    match config.unstaking_duration {
        None => {
            let msg = CosmosMsg::Bank(BankMsg::Send {
                to_address: info.sender.to_string(),
                amount: coins(amount.u128(), config.denom),
            });
            Ok(Response::new()
                .add_message(msg)
                .add_attribute("action", "unstake")
                .add_attribute("from", info.sender)
                .add_attribute("amount", amount)
                .add_attribute("claim_duration", "None"))
        }
        Some(duration) => {
            let outstanding_claims = CLAIMS.query_claims(deps.as_ref(), &info.sender)?.claims;
            if outstanding_claims.len() >= MAX_CLAIMS as usize {
                return Err(ContractError::TooManyClaims {});
            }

            CLAIMS.create_claim(
                deps.storage,
                &info.sender,
                amount,
                duration.after(&env.block),
            )?;
            Ok(Response::new()
                .add_attribute("action", "unstake")
                .add_attribute("from", info.sender)
                .add_attribute("amount", amount)
                .add_attribute("claim_duration", format!("{}", duration)))
        }
    }
}

pub fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    new_owner: Option<String>,
    new_manager: Option<String>,
    duration: Option<Duration>,
) -> Result<Response, ContractError> {
    let new_owner = new_owner
        .map(|new_owner| deps.api.addr_validate(&*new_owner))
        .transpose()?;
    let new_manager = new_manager
        .map(|new_manager| deps.api.addr_validate(&*new_manager))
        .transpose()?;
    let mut config: Config = CONFIG.load(deps.storage)?;
    if Some(info.sender.clone()) != config.owner && Some(info.sender.clone()) != config.manager {
        return Err(ContractError::Unauthorized {});
    };
    if Some(info.sender) != config.owner && new_owner != config.owner {
        return Err(ContractError::OnlyOwnerCanChangeOwner {});
    };

    config.owner = new_owner;
    config.manager = new_manager;

    config.unstaking_duration = duration;

    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new()
        .add_attribute("action", "update_config")
        .add_attribute(
            "owner",
            config
                .owner
                .map(|a| a.to_string())
                .unwrap_or_else(|| "None".to_string()),
        )
        .add_attribute(
            "manager",
            config
                .manager
                .map(|a| a.to_string())
                .unwrap_or_else(|| "None".to_string()),
        ))
}

pub fn execute_claim(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let release = CLAIMS.claim_tokens(deps.storage, &info.sender, &env.block, None)?;
    if release.is_zero() {
        return Err(ContractError::NothingToClaim {});
    }

    let config = CONFIG.load(deps.storage)?;
    let msg = CosmosMsg::Bank(BankMsg::Send {
        to_address: info.sender.to_string(),
        amount: coins(release.u128(), config.denom),
    });

    Ok(Response::new()
        .add_message(msg)
        .add_attribute("action", "claim")
        .add_attribute("from", info.sender)
        .add_attribute("amount", release))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::VotingPowerAtHeight { address, height } => {
            to_binary(&query_voting_power_at_height(deps, env, address, height)?)
        }
        QueryMsg::TotalPowerAtHeight { height } => {
            to_binary(&query_total_power_at_height(deps, env, height)?)
        }
        QueryMsg::Info {} => query_info(deps),
        QueryMsg::Dao {} => query_dao(deps),
        QueryMsg::Claims { address } => to_binary(&query_claims(deps, address)?),
        QueryMsg::GetConfig {} => to_binary(&query_config(deps)?),
    }
}

pub fn query_voting_power_at_height(
    deps: Deps,
    env: Env,
    address: String,
    height: Option<u64>,
) -> StdResult<VotingPowerAtHeightResponse> {
    let height = height.unwrap_or(env.block.height);
    let address = deps.api.addr_validate(&address)?;
    let power = STAKED_BALANCES
        .may_load_at_height(deps.storage, &address, height)?
        .unwrap_or_default();
    Ok(VotingPowerAtHeightResponse { power, height })
}

pub fn query_total_power_at_height(
    deps: Deps,
    env: Env,
    height: Option<u64>,
) -> StdResult<TotalPowerAtHeightResponse> {
    let height = height.unwrap_or(env.block.height);
    let power = STAKED_TOTAL
        .may_load_at_height(deps.storage, height)?
        .unwrap_or_default();
    Ok(TotalPowerAtHeightResponse { power, height })
}

pub fn query_info(deps: Deps) -> StdResult<Binary> {
    let info = cw2::get_contract_version(deps.storage)?;
    to_binary(&cw_core_interface::voting::InfoResponse { info })
}

pub fn query_dao(deps: Deps) -> StdResult<Binary> {
    let dao = DAO.load(deps.storage)?;
    to_binary(&dao)
}

pub fn query_config(deps: Deps) -> StdResult<GetConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    Ok(GetConfigResponse {
        owner: config.owner.map(|a| a.to_string()),
        manager: config.manager.map(|a| a.to_string()),
        unstaking_duration: config.unstaking_duration,
        denom: config.denom,
    })
}

pub fn query_claims(deps: Deps, address: String) -> StdResult<ClaimsResponse> {
    CLAIMS.query_claims(deps, &deps.api.addr_validate(&address)?)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    // Don't do any state migrations.
    Ok(Response::default())
}
