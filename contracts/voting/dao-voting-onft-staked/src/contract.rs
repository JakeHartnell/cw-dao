#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_json_binary, Addr, Binary, CosmosMsg, Deps, DepsMut, Empty, Env, MessageInfo, Response,
    StdResult, SubMsg, Uint128, Uint256,
};
use cw2::{get_contract_version, set_contract_version, ContractVersion};
use cw_storage_plus::Bound;
use cw_utils::Duration;
use dao_hooks::nft_stake::{stake_nft_hook_msgs, unstake_nft_hook_msgs};
use dao_interface::voting::IsActiveResponse;
use dao_voting::duration::validate_duration;
use dao_voting::threshold::{
    assert_valid_absolute_count_threshold, assert_valid_percentage_threshold, ActiveThreshold,
    ActiveThresholdResponse,
};

use crate::msg::{
    ClaimType, ExecuteMsg, InstantiateMsg, MigrateMsg, NftClaim, NftClaimsResponse, OnftCollection,
    QueryMsg,
};
use crate::omniflix::{get_onft_transfer_msg, query_onft_owner, query_onft_supply};
use crate::state::{
    register_staked_nfts, register_unstaked_nfts, Config, ACTIVE_THRESHOLD, CONFIG, DAO, HOOKS,
    LEGACY_NFT_CLAIMS, NFT_BALANCES, NFT_CLAIMS, PREPARED_ONFTS, STAKED_NFTS_PER_OWNER,
    TOTAL_STAKED_NFTS,
};
use crate::ContractError;

pub(crate) const CONTRACT_NAME: &str = "crates.io:dao-voting-onft-staked";
pub(crate) const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

// We multiply by this when calculating needed power for being active
// when using active threshold with percent
const PRECISION_FACTOR: u128 = 10u128.pow(9);

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response<Empty>, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    DAO.save(deps.storage, &info.sender)?;

    // Validate unstaking duration
    validate_duration(msg.unstaking_duration)?;

    // Validate active threshold if configured
    if let Some(active_threshold) = msg.active_threshold.as_ref() {
        match active_threshold {
            ActiveThreshold::Percentage { percent } => {
                assert_valid_percentage_threshold(*percent)?;
            }
            ActiveThreshold::AbsoluteCount { count } => {
                // Check absolute count is less than the supply of NFTs for
                // existing NFT collection.

                let OnftCollection::Existing { ref id } = msg.onft_collection;
                let nft_supply = query_onft_supply(deps.as_ref(), id)?;

                // Check the absolute count is less than the supply of NFTs and
                // greater than zero.
                assert_valid_absolute_count_threshold(*count, Uint128::new(nft_supply.into()))?;
            }
        }
        ACTIVE_THRESHOLD.save(deps.storage, active_threshold)?;
    }

    TOTAL_STAKED_NFTS.save(deps.storage, &Uint128::zero(), env.block.height)?;

    match msg.onft_collection {
        OnftCollection::Existing { id } => {
            let config = Config {
                onft_collection_id: id.clone(),
                unstaking_duration: msg.unstaking_duration,
            };
            CONFIG.save(deps.storage, &config)?;

            Ok(Response::default()
                .add_attribute("method", "instantiate")
                .add_attribute("onft_collection_id", id))
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
        ExecuteMsg::PrepareStake { token_ids } => execute_prepare_stake(deps, info, token_ids),
        ExecuteMsg::ConfirmStake { token_ids } => execute_confirm_stake(deps, env, info, token_ids),
        ExecuteMsg::CancelStake {
            token_ids,
            recipient,
        } => execute_cancel_stake(deps, env, info, token_ids, recipient),
        ExecuteMsg::Unstake { token_ids } => execute_unstake(deps, env, info, token_ids),
        ExecuteMsg::ClaimNfts { r#type } => execute_claim_nfts(deps, env, info, r#type),
        ExecuteMsg::UpdateConfig { duration } => execute_update_config(info, deps, duration),
        ExecuteMsg::AddHook { addr } => execute_add_hook(deps, info, addr),
        ExecuteMsg::RemoveHook { addr } => execute_remove_hook(deps, info, addr),
        ExecuteMsg::UpdateActiveThreshold { new_threshold } => {
            execute_update_active_threshold(deps, env, info, new_threshold)
        }
    }
}

pub fn execute_prepare_stake(
    deps: DepsMut,
    info: MessageInfo,
    token_ids: Vec<String>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // verify sender owns all the tokens
    let owns_all = token_ids
        .iter()
        .map(|token_id| -> StdResult<bool> {
            let owner = query_onft_owner(deps.as_ref(), &config.onft_collection_id, token_id)?;

            Ok(owner == info.sender)
        })
        .collect::<StdResult<Vec<bool>>>()?
        .into_iter()
        .all(|b| b);

    if !owns_all {
        return Err(ContractError::OnlyOwnerCanPrepareStake {});
    }

    // save and override prepared ONFTS, readying them to be transferred and
    // staked
    for token_id in &token_ids {
        PREPARED_ONFTS.save(deps.storage, token_id.to_string(), &info.sender)?;
    }

    Ok(Response::default()
        .add_attribute("action", "prepare_stake")
        .add_attribute("preparer", info.sender.to_string())
        .add_attribute("token_ids", token_ids.join(",")))
}

pub fn execute_confirm_stake(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    mut token_ids: Vec<String>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // de-duplicate token IDs to prevent double-counting exploit
    token_ids.sort();
    token_ids.dedup();

    // verify sender prepared and transferred all the tokens
    let sender_prepared_all = token_ids
        .iter()
        .map(|token_id| -> StdResult<bool> {
            // check if sender prepared
            let prepared = PREPARED_ONFTS
                .may_load(deps.storage, token_id.to_string())?
                .as_ref()
                == Some(&info.sender);

            // check that NFT was transferred to this contract
            let owner = query_onft_owner(deps.as_ref(), &config.onft_collection_id, token_id)?;

            Ok(prepared && owner == env.contract.address)
        })
        .collect::<StdResult<Vec<bool>>>()?
        .into_iter()
        .all(|b| b);

    if !sender_prepared_all {
        return Err(ContractError::StakeMustBePrepared {});
    }

    register_staked_nfts(deps.storage, env.block.height, &info.sender, &token_ids)?;

    let hook_msgs = token_ids
        .iter()
        .map(|token_id| {
            stake_nft_hook_msgs(HOOKS, deps.storage, info.sender.clone(), token_id.clone())
        })
        .collect::<StdResult<Vec<Vec<SubMsg>>>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<SubMsg>>();

    Ok(Response::default()
        .add_submessages(hook_msgs)
        .add_attribute("action", "stake")
        .add_attribute("from", info.sender)
        .add_attribute("token_ids", token_ids.join(",")))
}

/// CancelStake serves as an undo function in case an NFT or stake gets into a
/// bad state, either because the stake process was never completed, or because
/// someone sent an NFT to the staking contract without preparing the stake
/// first.
///
/// If called by:
/// - the original stake preparer, the preparation will be canceled, and the
///   NFT(s) will be sent back if the staking contract owns them.
/// - the current NFT(s) owner, the preparation will be canceled, if any.
/// - the DAO, the preparation will be canceled (if any exists), and the NFT(s)
///   will be sent to the specified recipient (if the staking contract owns
///   them). if no recipient is specified but the NFT was prepared, it will be
///   sent back to the preparer.
///
/// The recipient field only applies when the sender is the DAO. In the other
/// cases, the NFT(s) will always be sent back to the sender. Note: if the NFTs
/// were sent to the staking contract, but no stake was prepared, only the DAO
/// will be able to correct this and send them somewhere.
pub fn execute_cancel_stake(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    token_ids: Vec<String>,
    recipient: Option<String>,
) -> Result<Response, ContractError> {
    let dao = DAO.load(deps.storage)?;
    let config = CONFIG.load(deps.storage)?;

    // get preparers and owners of NFTs
    let token_ids_with_owners_and_preparers = token_ids
        .iter()
        .map(|token_id| {
            let preparer = PREPARED_ONFTS.may_load(deps.storage, token_id.clone())?;

            let owner = query_onft_owner(deps.as_ref(), &config.onft_collection_id, token_id)?;

            Ok((token_id, owner, preparer))
        })
        .collect::<StdResult<Vec<(&String, String, Option<Addr>)>>>()?;

    let mut transfer_msgs: Vec<CosmosMsg> = vec![];

    // If DAO, cancel preparations (if any) and send NFTs to the specified
    // recipient.
    if info.sender == dao {
        for (token_id, owner, preparer) in token_ids_with_owners_and_preparers {
            // cancel preparation if it exists
            if preparer.is_some() {
                PREPARED_ONFTS.remove(deps.storage, token_id.to_string());
            }

            // if this contract owns the NFT, send it to the recipient (or
            // preparer if one exists and no recipient was specified).
            if owner == env.contract.address {
                let recipient = recipient
                    .clone()
                    .or_else(|| preparer.map(|p| p.to_string()));

                if let Some(recipient) = recipient {
                    transfer_msgs.push(get_onft_transfer_msg(
                        &config.onft_collection_id,
                        token_id,
                        env.contract.address.as_str(),
                        &recipient,
                    ));
                } else {
                    return Err(ContractError::NoRecipient {});
                }
            }
        }
    } else {
        for (token_id, owner, preparer) in token_ids_with_owners_and_preparers {
            let is_preparer = preparer.as_ref() == Some(&info.sender);
            // only owner or preparer can cancel stake
            if info.sender != owner && !is_preparer {
                return Err(ContractError::NotPreparerNorOwner {});
            }

            // cancel preparation
            PREPARED_ONFTS.remove(deps.storage, token_id.to_string());

            // if owner is this staking contract, send it back to the preparer,
            // who must also be the sender (but let's force unwrap the preparer
            // just to make sure)
            if owner == env.contract.address {
                transfer_msgs.push(get_onft_transfer_msg(
                    &config.onft_collection_id,
                    token_id,
                    env.contract.address.as_str(),
                    preparer.unwrap().as_ref(),
                ));
            }
        }
    }

    Ok(Response::default()
        .add_messages(transfer_msgs)
        .add_attribute("action", "cancel_stake")
        .add_attribute("sender", info.sender)
        .add_attribute("token_ids", token_ids.join(","))
        .add_attribute(
            "recipient",
            recipient.unwrap_or_else(|| "_none".to_string()),
        ))
}

pub fn execute_unstake(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    token_ids: Vec<String>,
) -> Result<Response, ContractError> {
    if token_ids.is_empty() {
        return Err(ContractError::ZeroUnstake {});
    }

    register_unstaked_nfts(deps.storage, env.block.height, &info.sender, &token_ids)?;

    // Provided that the backing cw721 contract is non-malicious:
    //
    // 1. no token that has been staked may be staked again before
    //    first being unstaked.
    //
    // Provided that the other methods on this contract are functional:
    //
    // 2. there will never exist a pending claim for a token that is
    //    unstaked.
    // 3. (6) => claims may only be created for tokens that are staked.
    // 4. (1) && (2) && (3) => there will never be a staked NFT for
    //    which there is also a pending claim.
    //
    // (aside: the requirement on (1) for (4) may be confusing. it is
    // needed because if a token could be staked more than once, a
    // token could be staked, moved into the claims queue, and then
    // staked again, in which case the token is both staked and has a
    // pending claim.)
    //
    // If we reach this point in execution, `register_unstaked_nfts`
    // has not errored and thus:
    //
    // 5. token_ids contains no duplicate values.
    // 6. all NFTs in token_ids were staked by `info.sender`
    // 7. (4) && (6) => none of the tokens in token_ids are in the
    //    claims queue for `info.sender`
    //
    // (5) && (7) are the invariants for calling `create_nft_claims`
    // so if we reach this point in execution, we may safely create
    // claims.

    let hook_msgs =
        unstake_nft_hook_msgs(HOOKS, deps.storage, info.sender.clone(), token_ids.clone())?;

    let config = CONFIG.load(deps.storage)?;
    match config.unstaking_duration {
        None => {
            let return_messages = token_ids
                .into_iter()
                .map(|token_id| -> CosmosMsg {
                    get_onft_transfer_msg(
                        &config.onft_collection_id,
                        &token_id,
                        env.contract.address.as_str(),
                        info.sender.as_str(),
                    )
                })
                .collect::<Vec<_>>();

            Ok(Response::default()
                .add_messages(return_messages)
                .add_submessages(hook_msgs)
                .add_attribute("action", "unstake")
                .add_attribute("from", info.sender)
                .add_attribute("claim_duration", "None"))
        }

        Some(duration) => {
            // Out of gas here is fine - just try again with fewer
            // tokens.
            NFT_CLAIMS.create_nft_claims(
                deps.storage,
                &info.sender,
                token_ids,
                duration.after(&env.block),
            )?;

            Ok(Response::default()
                .add_attribute("action", "unstake")
                .add_submessages(hook_msgs)
                .add_attribute("from", info.sender)
                .add_attribute("claim_duration", format!("{duration}")))
        }
    }
}

pub fn execute_claim_nfts(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    claim_type: ClaimType,
) -> Result<Response, ContractError> {
    let token_ids = match claim_type {
        // attempt to claim all legacy NFTs
        ClaimType::Legacy => {
            LEGACY_NFT_CLAIMS.claim_nfts(deps.storage, &info.sender, &env.block)?
        }
        // attempt to claim all non-legacy NFTs
        ClaimType::All => {
            let token_ids = NFT_CLAIMS
                .query_claims(deps.as_ref(), &info.sender, None, None)?
                .into_iter()
                .filter(|nft| nft.release_at.is_expired(&env.block))
                .map(|nft| nft.token_id)
                .collect::<Vec<_>>();

            if !token_ids.is_empty() {
                NFT_CLAIMS.claim_nfts(deps.storage, &info.sender, &token_ids, &env.block)?;
            }

            token_ids
        }
        // attempt to claim specific non-legacy NFTs
        ClaimType::Specific(token_ids) => {
            if !token_ids.is_empty() {
                NFT_CLAIMS.claim_nfts(deps.storage, &info.sender, &token_ids, &env.block)?;
            }

            token_ids
        }
    };

    if token_ids.is_empty() {
        return Err(ContractError::NothingToClaim {});
    }

    let config = CONFIG.load(deps.storage)?;

    let msgs = token_ids
        .into_iter()
        .map(|token_id| -> CosmosMsg {
            get_onft_transfer_msg(
                &config.onft_collection_id,
                &token_id,
                env.contract.address.as_str(),
                info.sender.as_str(),
            )
        })
        .collect::<Vec<_>>();

    Ok(Response::default()
        .add_messages(msgs)
        .add_attribute("action", "claim_nfts")
        .add_attribute("from", info.sender))
}

pub fn execute_update_config(
    info: MessageInfo,
    deps: DepsMut,
    duration: Option<Duration>,
) -> Result<Response, ContractError> {
    let mut config: Config = CONFIG.load(deps.storage)?;
    let dao = DAO.load(deps.storage)?;

    // Only the DAO can update the config.
    if info.sender != dao {
        return Err(ContractError::Unauthorized {});
    }

    // Validate unstaking duration
    validate_duration(duration)?;

    config.unstaking_duration = duration;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::default()
        .add_attribute("action", "update_config")
        .add_attribute(
            "unstaking_duration",
            config
                .unstaking_duration
                .map(|d| d.to_string())
                .unwrap_or_else(|| "none".to_string()),
        ))
}

pub fn execute_add_hook(
    deps: DepsMut,
    info: MessageInfo,
    addr: String,
) -> Result<Response, ContractError> {
    let dao = DAO.load(deps.storage)?;

    // Only the DAO can add a hook
    if info.sender != dao {
        return Err(ContractError::Unauthorized {});
    }

    let hook = deps.api.addr_validate(&addr)?;
    HOOKS.add_hook(deps.storage, hook)?;

    Ok(Response::default()
        .add_attribute("action", "add_hook")
        .add_attribute("hook", addr))
}

pub fn execute_remove_hook(
    deps: DepsMut,
    info: MessageInfo,
    addr: String,
) -> Result<Response, ContractError> {
    let dao = DAO.load(deps.storage)?;

    // Only the DAO can remove a hook
    if info.sender != dao {
        return Err(ContractError::Unauthorized {});
    }

    let hook = deps.api.addr_validate(&addr)?;
    HOOKS.remove_hook(deps.storage, hook)?;

    Ok(Response::default()
        .add_attribute("action", "remove_hook")
        .add_attribute("hook", addr))
}

pub fn execute_update_active_threshold(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    new_active_threshold: Option<ActiveThreshold>,
) -> Result<Response, ContractError> {
    let dao = DAO.load(deps.storage)?;
    if info.sender != dao {
        return Err(ContractError::Unauthorized {});
    }

    let config = CONFIG.load(deps.storage)?;
    if let Some(active_threshold) = new_active_threshold {
        match active_threshold {
            ActiveThreshold::Percentage { percent } => {
                assert_valid_percentage_threshold(percent)?;
            }
            ActiveThreshold::AbsoluteCount { count } => {
                let nft_supply = query_onft_supply(deps.as_ref(), &config.onft_collection_id)?;
                assert_valid_absolute_count_threshold(count, Uint128::new(nft_supply.into()))?;
            }
        }
        ACTIVE_THRESHOLD.save(deps.storage, &active_threshold)?;
    } else {
        ACTIVE_THRESHOLD.remove(deps.storage);
    }

    Ok(Response::new().add_attribute("action", "update_active_threshold"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::ActiveThreshold {} => query_active_threshold(deps),
        QueryMsg::Config {} => query_config(deps),
        QueryMsg::Dao {} => query_dao(deps),
        QueryMsg::Info {} => query_info(deps),
        QueryMsg::IsActive {} => query_is_active(deps, env),
        QueryMsg::NftClaims {
            address,
            start_after,
            limit,
        } => query_nft_claims(deps, address, start_after, limit),
        QueryMsg::Hooks {} => query_hooks(deps),
        QueryMsg::StakedNfts {
            address,
            start_after,
            limit,
        } => query_staked_nfts(deps, address, start_after, limit),
        QueryMsg::TotalPowerAtHeight { height } => query_total_power_at_height(deps, env, height),
        QueryMsg::VotingPowerAtHeight { address, height } => {
            query_voting_power_at_height(deps, env, address, height)
        }
    }
}

pub fn query_active_threshold(deps: Deps) -> StdResult<Binary> {
    to_json_binary(&ActiveThresholdResponse {
        active_threshold: ACTIVE_THRESHOLD.may_load(deps.storage)?,
    })
}

pub fn query_is_active(deps: Deps, env: Env) -> StdResult<Binary> {
    let threshold = ACTIVE_THRESHOLD.may_load(deps.storage)?;
    if let Some(threshold) = threshold {
        let config = CONFIG.load(deps.storage)?;
        let staked_nfts = TOTAL_STAKED_NFTS
            .may_load_at_height(deps.storage, env.block.height)?
            .unwrap_or_default();
        let total_nfts = query_onft_supply(deps, &config.onft_collection_id)?;

        match threshold {
            ActiveThreshold::AbsoluteCount { count } => to_json_binary(&IsActiveResponse {
                active: staked_nfts >= count,
            }),
            ActiveThreshold::Percentage { percent } => {
                // Check if there are any staked NFTs
                if staked_nfts.is_zero() {
                    return to_json_binary(&IsActiveResponse { active: false });
                }

                // percent is bounded between [0, 100]. decimal
                // represents percents in u128 terms as p *
                // 10^15. this bounds percent between [0, 10^17].
                //
                // total_potential_power is bounded between [0, 2^64]
                // as it tracks the count of NFT tokens which has
                // a max supply of 2^64.
                //
                // with our precision factor being 10^9:
                //
                // total_nfts <= 2^64 * 10^9 <= 2^256
                //
                // so we're good to put that in a u256.
                //
                // multiply_ratio promotes to a u512 under the hood,
                // so it won't overflow, multiplying by a percent less
                // than 100 is gonna make something the same size or
                // smaller, applied + 10^9 <= 2^128 * 10^9 + 10^9 <=
                // 2^256, so the top of the round won't overflow, and
                // rounding is rounding down, so the whole thing can
                // be safely unwrapped at the end of the day thank you
                // for coming to my ted talk.
                let total_nfts_count = Uint128::from(total_nfts).full_mul(PRECISION_FACTOR);

                // under the hood decimals are `atomics / 10^decimal_places`.
                // cosmwasm doesn't give us a Decimal * Uint256
                // implementation so we take the decimal apart and
                // multiply by the fraction.
                let applied = total_nfts_count.multiply_ratio(
                    percent.atomics(),
                    Uint256::from(10u64).pow(percent.decimal_places()),
                );
                let rounded = (applied + Uint256::from(PRECISION_FACTOR) - Uint256::from(1u128))
                    / Uint256::from(PRECISION_FACTOR);
                let count: Uint128 = rounded.try_into().unwrap();

                // staked_nfts >= total_nfts * percent
                to_json_binary(&IsActiveResponse {
                    active: staked_nfts >= count,
                })
            }
        }
    } else {
        to_json_binary(&IsActiveResponse { active: true })
    }
}

pub fn query_voting_power_at_height(
    deps: Deps,
    env: Env,
    address: String,
    height: Option<u64>,
) -> StdResult<Binary> {
    let address = deps.api.addr_validate(&address)?;
    let height = height.unwrap_or(env.block.height);
    let power = NFT_BALANCES
        .may_load_at_height(deps.storage, &address, height)?
        .unwrap_or_default();
    to_json_binary(&dao_interface::voting::VotingPowerAtHeightResponse { power, height })
}

pub fn query_total_power_at_height(deps: Deps, env: Env, height: Option<u64>) -> StdResult<Binary> {
    let height = height.unwrap_or(env.block.height);
    let power = TOTAL_STAKED_NFTS
        .may_load_at_height(deps.storage, height)?
        .unwrap_or_default();
    to_json_binary(&dao_interface::voting::TotalPowerAtHeightResponse { power, height })
}

pub fn query_config(deps: Deps) -> StdResult<Binary> {
    let config = CONFIG.load(deps.storage)?;
    to_json_binary(&config)
}

pub fn query_dao(deps: Deps) -> StdResult<Binary> {
    let dao = DAO.load(deps.storage)?;
    to_json_binary(&dao)
}

pub fn query_nft_claims(
    deps: Deps,
    address: String,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<Binary> {
    let addr = deps.api.addr_validate(&address)?;

    // load all legacy claims since it does not support pagination
    let legacy_claims = LEGACY_NFT_CLAIMS
        .query_claims(deps, &addr)?
        .nft_claims
        .into_iter()
        .map(|c| NftClaim {
            token_id: c.token_id,
            release_at: c.release_at,
            legacy: true,
        })
        .collect::<Vec<_>>();

    // paginate all new claims
    let claims = NFT_CLAIMS
        .query_claims(deps, &addr, start_after.as_ref(), limit)?
        .into_iter()
        .map(|c| NftClaim {
            token_id: c.token_id,
            release_at: c.release_at,
            legacy: false,
        })
        .collect::<Vec<_>>();

    // combine legacy and new claims
    let nft_claims = legacy_claims.into_iter().chain(claims).collect();

    to_json_binary(&NftClaimsResponse { nft_claims })
}

pub fn query_hooks(deps: Deps) -> StdResult<Binary> {
    to_json_binary(&HOOKS.query_hooks(deps)?)
}

pub fn query_info(deps: Deps) -> StdResult<Binary> {
    let info = cw2::get_contract_version(deps.storage)?;
    to_json_binary(&dao_interface::voting::InfoResponse { info })
}

pub fn query_staked_nfts(
    deps: Deps,
    address: String,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<Binary> {
    let prefix = deps.api.addr_validate(&address)?;
    let prefix = STAKED_NFTS_PER_OWNER.prefix(&prefix);

    let start_after = start_after.as_deref().map(Bound::exclusive);
    let range = prefix.keys(
        deps.storage,
        start_after,
        None,
        cosmwasm_std::Order::Ascending,
    );
    let range: StdResult<Vec<String>> = match limit {
        Some(l) => range.take(l as usize).collect(),
        None => range.collect(),
    };
    to_json_binary(&range?)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    let storage_version: ContractVersion = get_contract_version(deps.storage)?;

    // Only migrate if newer
    if storage_version.version.as_str() < CONTRACT_VERSION {
        // Set contract to version to latest
        set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    }

    Ok(Response::new().add_attribute("action", "migrate"))
}
