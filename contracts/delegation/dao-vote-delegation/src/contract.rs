#[cfg(not(feature = "library"))]
use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Decimal, Deps, DepsMut, Env, MessageInfo, Response,
    StdResult, Uint128,
};
use cosmwasm_std::{Addr, Order};
use cw2::{get_contract_version, set_contract_version};
use cw_paginate_storage::paginate_map_keys;
use cw_storage_plus::Bound;
use cw_utils::{maybe_addr, nonpayable};
use dao_interface::helpers::OptionalUpdate;
use dao_interface::state::{ProposalModule, ProposalModuleStatus};
use dao_interface::voting::InfoResponse;
use dao_voting::delegation::{
    calculate_delegated_vp, DelegationResponse, RegistrationResponse,
    UnvotedDelegatedVotingPowerResponse,
};
use dao_voting::voting;
use semver::Version;

use crate::helpers::{
    add_delegated_vp, ensure_setup, get_udvp, get_voting_power, is_delegate_registered,
    remove_delegated_vp, unregister_delegate,
};
use crate::hooks::{
    execute_membership_changed, execute_nft_stake_changed, execute_stake_changed, execute_vote_hook,
};
use crate::msg::{
    DelegateResponse, DelegatesResponse, DelegationsResponse, ExecuteMsg, InstantiateMsg,
    MigrateMsg, QueryMsg,
};
use crate::state::{
    Config, Delegate, Delegation, CONFIG, DAO, DELEGATED_VP, DELEGATES, DELEGATIONS,
    DELEGATION_ENTRIES, PERCENT_DELEGATED, PROPOSAL_HOOK_CALLERS, VOTING_POWER_HOOK_CALLERS,
    VP_CAP_PERCENT,
};
use crate::ContractError;

pub(crate) const CONTRACT_NAME: &str = "crates.io:dao-vote-delegation";
pub(crate) const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub const DEFAULT_LIMIT: u32 = 10;

/// in tests on Neutron, with a block max gas of 30M (which is one of the lowest
/// gas limits on any chain), we found that 50 delegations is a safe upper
/// bound, so this defaults to 50.
pub const DEFAULT_MAX_DELEGATIONS: u64 = 50;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let dao = msg
        .dao
        .map(|d| deps.api.addr_validate(&d))
        .transpose()?
        .unwrap_or(info.sender);

    DAO.save(deps.storage, &dao)?;

    if let Some(delegation_validity_blocks) = msg.delegation_validity_blocks {
        if delegation_validity_blocks < 2 {
            return Err(ContractError::InvalidDelegationValidityBlocks {
                provided: delegation_validity_blocks,
                min: 2,
            });
        }
    }

    CONFIG.save(
        deps.storage,
        &Config {
            delegation_validity_blocks: msg.delegation_validity_blocks,
            max_delegations: msg.max_delegations.unwrap_or(DEFAULT_MAX_DELEGATIONS),
        },
    )?;
    VP_CAP_PERCENT.save(deps.storage, &msg.vp_cap_percent, env.block.height)?;

    // initialize voting power changed hook callers
    if let Some(vp_hook_callers) = msg.vp_hook_callers {
        for caller in vp_hook_callers {
            VOTING_POWER_HOOK_CALLERS.save(deps.storage, deps.api.addr_validate(&caller)?, &())?;
        }
    }

    // sync proposal modules with no limit if not disabled. this should succeed
    // for most DAOs as the query will not run out of gas with only a few
    // proposal modules.
    if !msg.no_sync_proposal_modules.unwrap_or(false) {
        execute_sync_proposal_modules(deps, None, None)?;
    }

    Ok(Response::new().add_attribute("dao", dao))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Register {} => execute_register(deps, env, info),
        ExecuteMsg::Unregister {} => execute_unregister(deps, env, info),
        ExecuteMsg::Delegate { delegate, percent } => {
            execute_delegate(deps, env, info, delegate, percent)
        }
        ExecuteMsg::Undelegate { delegate } => execute_undelegate(deps, env, info, delegate),
        ExecuteMsg::UpdateVotingPowerHookCallers { add, remove } => {
            execute_update_voting_power_hook_callers(deps, info, add, remove)
        }
        ExecuteMsg::SyncProposalModules { start_after, limit } => {
            execute_sync_proposal_modules(deps, start_after, limit)
        }
        ExecuteMsg::UpdateConfig {
            vp_cap_percent,
            delegation_validity_blocks,
            max_delegations,
        } => execute_update_config(
            deps,
            env,
            info,
            vp_cap_percent,
            delegation_validity_blocks,
            max_delegations,
        ),
        ExecuteMsg::StakeChangeHook(msg) => execute_stake_changed(deps, env, info, msg),
        ExecuteMsg::NftStakeChangeHook(msg) => execute_nft_stake_changed(deps, env, info, msg),
        ExecuteMsg::MemberChangedHook(msg) => execute_membership_changed(deps, env, info, msg),
        ExecuteMsg::VoteHook(vote_hook) => execute_vote_hook(deps, env, info, vote_hook),
    }
}

fn execute_register(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    nonpayable(&info)?;
    ensure_setup(deps.as_ref())?;

    let delegate = info.sender;

    if is_delegate_registered(deps.as_ref(), &delegate, None)? {
        return Err(ContractError::DelegateAlreadyRegistered {});
    }

    // ensure delegate has voting power in the DAO
    let vp = get_voting_power(
        deps.as_ref(),
        &delegate,
        // use next block height since voting power takes effect at the start of
        // the next block. if the delegate changed their voting power in the
        // current block, we need to use the new value.
        env.block.height + 1,
    )?;
    if vp.is_zero() {
        return Err(ContractError::NoVotingPower {});
    }

    // ensure delegate has no delegations
    let has_delegations = !DELEGATION_ENTRIES.prefix(&delegate).is_empty(deps.storage);
    if has_delegations {
        return Err(ContractError::CannotRegisterWithDelegations {});
    }

    DELEGATES.save(deps.storage, delegate, &Delegate {}, env.block.height)?;

    Ok(Response::new())
}

fn execute_unregister(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    nonpayable(&info)?;
    ensure_setup(deps.as_ref())?;

    let delegate = info.sender;

    if !is_delegate_registered(deps.as_ref(), &delegate, None)? {
        return Err(ContractError::DelegateNotRegistered {});
    }

    unregister_delegate(deps, &delegate, env.block.height)?;

    Ok(Response::new())
}

fn execute_delegate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    delegate: String,
    percent: Decimal,
) -> Result<Response, ContractError> {
    nonpayable(&info)?;
    ensure_setup(deps.as_ref())?;

    if percent <= Decimal::zero() || percent > Decimal::one() {
        return Err(ContractError::InvalidVotingPowerPercent {});
    }

    let delegator = info.sender;
    let delegate = deps.api.addr_validate(&delegate)?;

    // delegates cannot delegate to others
    if is_delegate_registered(deps.as_ref(), &delegator, None)? {
        return Err(ContractError::DelegatesCannotDelegate {});
    }

    // ensure delegate is registered
    if !is_delegate_registered(deps.as_ref(), &delegate, None)? {
        return Err(ContractError::DelegateNotRegistered {});
    }

    // ensure delegator has voting power in the DAO
    let vp = get_voting_power(
        deps.as_ref(),
        &delegator,
        // use next block height since voting power takes effect at the start of
        // the next block. if the delegator changed their voting power in the
        // current block, we need to use the new value.
        env.block.height + 1,
    )?;
    if vp.is_zero() {
        return Err(ContractError::NoVotingPower {});
    }

    let config = CONFIG.load(deps.storage)?;

    let current_percent_delegated = PERCENT_DELEGATED
        .may_load(deps.storage, &delegator)?
        .unwrap_or_default();

    let existing_delegation_entry =
        DELEGATION_ENTRIES.may_load(deps.storage, (&delegator, &delegate))?;

    // will be set below, differing based on whether this is a new delegation or
    // an update to an existing one
    let new_total_percent_delegated: Decimal;

    // update an existing delegation
    if let Some((existing_id, existing_expiration)) = existing_delegation_entry {
        let mut existing_delegation =
            DELEGATIONS.load_item(deps.storage, &delegator, existing_id)?;

        // remove existing percent and replace with new percent
        new_total_percent_delegated = current_percent_delegated
            .checked_sub(existing_delegation.percent)?
            .checked_add(percent)?;

        // remove current delegated VP based on existing percent
        let current_delegated_vp = calculate_delegated_vp(vp, existing_delegation.percent);
        remove_delegated_vp(
            deps.storage,
            &env,
            &delegate,
            current_delegated_vp,
            existing_expiration,
        )?;

        // replace delegation with updated percent
        let (_, total_minus_one) =
            DELEGATIONS.remove(deps.storage, &delegator, existing_id, env.block.height)?;

        // don't let them update if they are over the max, instead requiring
        // them to remove existing delegations before updating any
        if total_minus_one + 1 > config.max_delegations as usize {
            return Err(ContractError::MaxDelegationsReached {
                max: config.max_delegations,
                current: total_minus_one + 1,
            });
        }

        existing_delegation.percent = percent;

        let (new_delegation_entry, _) = DELEGATIONS.push(
            deps.storage,
            &delegator,
            &existing_delegation,
            env.block.height,
            config.delegation_validity_blocks,
        )?;
        DELEGATION_ENTRIES.save(deps.storage, (&delegator, &delegate), &new_delegation_entry)?;
    }
    // create a new delegation
    else {
        new_total_percent_delegated = current_percent_delegated.checked_add(percent)?;

        // add new delegation
        let (new_delegation_entry, new_total) = DELEGATIONS.push(
            deps.storage,
            &delegator,
            &Delegation {
                delegate: delegate.clone(),
                percent,
            },
            env.block.height,
            config.delegation_validity_blocks,
        )?;

        // prevent new delegations if they are over the max
        if new_total > config.max_delegations as usize {
            return Err(ContractError::MaxDelegationsReached {
                max: config.max_delegations,
                current: new_total - 1,
            });
        }

        DELEGATION_ENTRIES.save(deps.storage, (&delegator, &delegate), &new_delegation_entry)?;
    }

    // ensure not delegating more than 100%
    if new_total_percent_delegated > Decimal::one() {
        return Err(ContractError::CannotDelegateMoreThan100Percent {
            current: current_percent_delegated
                .checked_mul(Decimal::from_atomics(100u128, 0).unwrap())?
                .to_string(),
            attempt: new_total_percent_delegated
                .checked_mul(Decimal::from_atomics(100u128, 0).unwrap())?
                .to_string(),
        });
    }

    PERCENT_DELEGATED.save(deps.storage, &delegator, &new_total_percent_delegated)?;

    // calculate the new delegated VP and add to the delegate's total
    let new_delegated_vp = calculate_delegated_vp(vp, percent);
    add_delegated_vp(
        deps.storage,
        &env,
        &delegate,
        new_delegated_vp,
        config.delegation_validity_blocks,
    )?;

    Ok(Response::new())
}

fn execute_undelegate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    delegate: String,
) -> Result<Response, ContractError> {
    nonpayable(&info)?;
    ensure_setup(deps.as_ref())?;

    let delegator = info.sender;
    let delegate = deps.api.addr_validate(&delegate)?;

    // ensure delegation exists
    let (existing_id, existing_expiration) = DELEGATION_ENTRIES
        .load(deps.storage, (&delegator, &delegate))
        .map_err(|_| ContractError::DelegationDoesNotExist {})?;

    // if delegation exists above, percent will exist
    let current_percent_delegated = PERCENT_DELEGATED.load(deps.storage, &delegator)?;

    // retrieve and remove delegation
    let (delegation, _) =
        DELEGATIONS.remove(deps.storage, &delegator, existing_id, env.block.height)?;
    DELEGATION_ENTRIES.remove(deps.storage, (&delegator, &delegate));

    // update delegator's percent delegated
    let new_percent_delegated = current_percent_delegated.checked_sub(delegation.percent)?;
    PERCENT_DELEGATED.save(deps.storage, &delegator, &new_percent_delegated)?;

    let vp = get_voting_power(
        deps.as_ref(),
        &delegator,
        // use next block height since voting power takes effect at the start of
        // the next block. if the delegator changed their voting power in the
        // current block, we need to use the new value.
        env.block.height + 1,
    )?;

    // remove delegated VP from delegate's total delegated VP at the current
    // height.
    let current_delegated_vp = calculate_delegated_vp(vp, delegation.percent);
    remove_delegated_vp(
        deps.storage,
        &env,
        &delegate,
        current_delegated_vp,
        existing_expiration,
    )?;

    Ok(Response::new().add_attribute("action", "undelegate"))
}

fn execute_update_voting_power_hook_callers(
    deps: DepsMut,
    info: MessageInfo,
    add: Option<Vec<String>>,
    remove: Option<Vec<String>>,
) -> Result<Response, ContractError> {
    nonpayable(&info)?;

    // only the DAO can update the voting power hook callers
    let dao = DAO.load(deps.storage)?;
    if info.sender != dao {
        return Err(ContractError::Unauthorized {});
    }

    if let Some(add) = add {
        for addr in add {
            VOTING_POWER_HOOK_CALLERS.save(deps.storage, deps.api.addr_validate(&addr)?, &())?;
        }
    }

    if let Some(remove) = remove {
        for addr in remove {
            VOTING_POWER_HOOK_CALLERS.remove(deps.storage, deps.api.addr_validate(&addr)?);
        }
    }

    Ok(Response::new().add_attribute("action", "update_voting_power_hook_callers"))
}

fn execute_sync_proposal_modules(
    deps: DepsMut,
    start_after: Option<String>,
    limit: Option<u32>,
) -> Result<Response, ContractError> {
    let dao = DAO.load(deps.storage)?;
    let proposal_modules: Vec<ProposalModule> = deps.querier.query_wasm_smart(
        dao,
        &dao_interface::msg::QueryMsg::ProposalModules { start_after, limit },
    )?;

    let mut enabled = 0;
    let mut disabled = 0;
    for proposal_module in proposal_modules {
        if proposal_module.status == ProposalModuleStatus::Enabled {
            enabled += 1;
            PROPOSAL_HOOK_CALLERS.save(deps.storage, proposal_module.address, &())?;
        } else {
            disabled += 1;
            PROPOSAL_HOOK_CALLERS.remove(deps.storage, proposal_module.address);
        }
    }

    Ok(Response::new()
        .add_attribute("action", "sync_proposal_modules")
        .add_attribute("enabled", enabled.to_string())
        .add_attribute("disabled", disabled.to_string()))
}

fn execute_update_config(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    vp_cap_percent: OptionalUpdate<Decimal>,
    delegation_validity_blocks: OptionalUpdate<u64>,
    max_delegations: Option<u64>,
) -> Result<Response, ContractError> {
    nonpayable(&info)?;

    // only the DAO can update the config
    let dao = DAO.load(deps.storage)?;
    if info.sender != dao {
        return Err(ContractError::Unauthorized {});
    }

    vp_cap_percent
        .maybe_update_result(|value| VP_CAP_PERCENT.save(deps.storage, &value, env.block.height))?;

    CONFIG.update(deps.storage, |mut config| -> Result<_, ContractError> {
        delegation_validity_blocks.maybe_update_result(|value| {
            // validate if defined
            if let Some(value) = value {
                if value < 2 {
                    return Err(ContractError::InvalidDelegationValidityBlocks {
                        provided: value,
                        min: 2,
                    });
                }
            }

            config.delegation_validity_blocks = value;

            Ok(())
        })?;

        if let Some(value) = max_delegations {
            config.max_delegations = value;
        }

        Ok(config)
    })?;

    Ok(Response::new().add_attribute("action", "update_config"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Info {} => Ok(to_json_binary(&query_info(deps)?)?),
        QueryMsg::Registration { delegate, height } => Ok(to_json_binary(&query_registration(
            deps, env, delegate, height,
        )?)?),
        QueryMsg::Delegates { start_after, limit } => Ok(to_json_binary(&query_delegates(
            deps,
            env,
            start_after,
            limit,
        )?)?),
        QueryMsg::Delegations {
            delegator,
            height,
            offset,
            limit,
        } => Ok(to_json_binary(&query_delegations(
            deps, env, delegator, height, offset, limit,
        )?)?),
        QueryMsg::UnvotedDelegatedVotingPower {
            delegate,
            proposal_module,
            proposal_id,
            height,
        } => Ok(to_json_binary(&query_unvoted_delegated_vp(
            deps,
            delegate,
            proposal_module,
            proposal_id,
            height,
        )?)?),
        QueryMsg::ProposalModules { start_after, limit } => Ok(to_json_binary(
            &query_proposal_modules(deps, start_after, limit)?,
        )?),
        QueryMsg::VotingPowerHookCallers { start_after, limit } => Ok(to_json_binary(
            &query_voting_power_hook_callers(deps, start_after, limit)?,
        )?),
        QueryMsg::Config {} => Ok(to_json_binary(&query_config(deps)?)?),
    }
}

fn query_info(deps: Deps) -> StdResult<InfoResponse> {
    let info = get_contract_version(deps.storage)?;
    Ok(InfoResponse { info })
}

fn query_registration(
    deps: Deps,
    env: Env,
    delegate: String,
    height: Option<u64>,
) -> StdResult<RegistrationResponse> {
    let height = height.unwrap_or(env.block.height);
    let delegate = deps.api.addr_validate(&delegate)?;

    let registered = is_delegate_registered(deps, &delegate, Some(height))?;
    let power = DELEGATED_VP
        .load(deps.storage, delegate, height)?
        .unwrap_or_default();

    Ok(RegistrationResponse {
        registered,
        power,
        height,
    })
}

fn query_delegates(
    deps: Deps,
    env: Env,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<DelegatesResponse> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT) as usize;

    let start = maybe_addr(deps.api, start_after)?.map(Bound::exclusive);

    let delegates = DELEGATES
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|delegate| {
            delegate.map(|(delegate, _)| -> StdResult<DelegateResponse> {
                let power = DELEGATED_VP
                    .load(deps.storage, delegate.clone(), env.block.height)?
                    .unwrap_or_default();
                Ok(DelegateResponse { delegate, power })
            })?
        })
        .collect::<StdResult<_>>()?;

    Ok(DelegatesResponse { delegates })
}

fn query_delegations(
    deps: Deps,
    env: Env,
    delegator: String,
    height: Option<u64>,
    offset: Option<u64>,
    limit: Option<u64>,
) -> StdResult<DelegationsResponse> {
    let height = height.unwrap_or(env.block.height);
    let delegator = deps.api.addr_validate(&delegator)?;
    let delegations = DELEGATIONS
        .load(deps.storage, &delegator, height, limit, offset)?
        .into_iter()
        .map(|d| -> StdResult<DelegationResponse> {
            let active = is_delegate_registered(deps, &d.item.delegate, Some(height))?;
            Ok(DelegationResponse {
                delegate: d.item.delegate,
                percent: d.item.percent,
                active,
            })
        })
        .collect::<StdResult<_>>()?;
    Ok(DelegationsResponse {
        delegations,
        height,
    })
}

fn query_unvoted_delegated_vp(
    deps: Deps,
    delegate: String,
    proposal_module: String,
    proposal_id: u64,
    height: u64,
) -> StdResult<UnvotedDelegatedVotingPowerResponse> {
    let delegate = deps.api.addr_validate(&delegate)?;

    // if delegate not registered, they have no unvoted delegated VP.
    if !is_delegate_registered(deps, &delegate, Some(height))? {
        return Ok(UnvotedDelegatedVotingPowerResponse {
            total: Uint128::zero(),
            effective: Uint128::zero(),
        });
    }

    let proposal_module = deps.api.addr_validate(&proposal_module)?;

    let total = get_udvp(deps, &delegate, &proposal_module, proposal_id, height)?;
    let mut effective = total;

    // if a VP cap is set, apply it to the total VP to get the effective VP.
    let vp_cap_percent = VP_CAP_PERCENT
        .may_load_at_height(deps.storage, height)?
        .unwrap_or(None);
    if let Some(vp_cap_percent) = vp_cap_percent {
        if vp_cap_percent < Decimal::one() {
            let dao = DAO.load(deps.storage)?;
            let total_power = voting::get_total_power(deps, &dao, Some(height))?;
            let cap = calculate_delegated_vp(total_power, vp_cap_percent);

            effective = total.min(cap);
        }
    }

    Ok(UnvotedDelegatedVotingPowerResponse { total, effective })
}

fn query_proposal_modules(
    deps: Deps,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<Vec<Addr>> {
    paginate_map_keys(
        deps,
        &PROPOSAL_HOOK_CALLERS,
        start_after
            .map(|s| deps.api.addr_validate(&s))
            .transpose()?,
        limit,
        Order::Ascending,
    )
}

fn query_voting_power_hook_callers(
    deps: Deps,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<Vec<Addr>> {
    paginate_map_keys(
        deps,
        &VOTING_POWER_HOOK_CALLERS,
        start_after
            .map(|s| deps.api.addr_validate(&s))
            .transpose()?,
        limit,
        Order::Ascending,
    )
}

fn query_config(deps: Deps) -> StdResult<Config> {
    let config = CONFIG.load(deps.storage)?;
    Ok(config)
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
        return Err(ContractError::MigrationErrorInvalidVersion {
            new: new_version.to_string(),
            current: current_version.to_string(),
        });
    }

    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    Ok(Response::default())
}
