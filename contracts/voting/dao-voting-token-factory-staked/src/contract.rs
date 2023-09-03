#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    coins, to_binary, BankMsg, BankQuery, Binary, Coin, CosmosMsg, Decimal, Deps, DepsMut, Env,
    MessageInfo, Order, Reply, Response, StdResult, SubMsg, Uint128, Uint256, WasmMsg,
};
use cw2::{get_contract_version, set_contract_version, ContractVersion};
use cw_controllers::ClaimsResponse;
use cw_storage_plus::Bound;
use cw_tokenfactory_issuer::msg::{
    DenomUnit, ExecuteMsg as IssuerExecuteMsg, InstantiateMsg as IssuerInstantiateMsg, Metadata,
};
use cw_utils::{maybe_addr, must_pay, parse_reply_instantiate_data, Duration};
use dao_hooks::stake::{stake_hook_msgs, unstake_hook_msgs};
use dao_interface::voting::{
    IsActiveResponse, TotalPowerAtHeightResponse, VotingPowerAtHeightResponse,
};
use dao_voting::threshold::{ActiveThreshold, ActiveThresholdResponse};

use crate::error::ContractError;

use crate::msg::{
    DenomResponse, ExecuteMsg, GetHooksResponse, InitialBalance, InstantiateMsg,
    ListStakersResponse, MigrateMsg, QueryMsg, StakerBalanceResponse, TokenInfo,
};
use crate::state::{
    Config, ACTIVE_THRESHOLD, CLAIMS, CONFIG, DAO, DENOM, HOOKS, MAX_CLAIMS, STAKED_BALANCES,
    STAKED_TOTAL, TOKEN_INSTANTIATION_INFO, TOKEN_ISSUER_CONTRACT,
};

pub(crate) const CONTRACT_NAME: &str = "crates.io:dao-voting-token-factory-staked";
pub(crate) const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

// Settings for query pagination
const MAX_LIMIT: u32 = 30;
const DEFAULT_LIMIT: u32 = 10;

const INSTANTIATE_TOKEN_FACTORY_ISSUER_REPLY_ID: u64 = 0;

// We multiply by this when calculating needed power for being active
// when using active threshold with percent
const PRECISION_FACTOR: u128 = 10u128.pow(9);

fn validate_duration(duration: Option<Duration>) -> Result<(), ContractError> {
    if let Some(unstaking_duration) = duration {
        match unstaking_duration {
            Duration::Height(height) => {
                if height == 0 {
                    return Err(ContractError::InvalidUnstakingDuration {});
                }
            }
            Duration::Time(time) => {
                if time == 0 {
                    return Err(ContractError::InvalidUnstakingDuration {});
                }
            }
        }
    }
    Ok(())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    validate_duration(msg.unstaking_duration)?;

    let config = Config {
        unstaking_duration: msg.unstaking_duration,
    };

    CONFIG.save(deps.storage, &config)?;
    DAO.save(deps.storage, &info.sender)?;

    // Validate Active Threshold
    if let Some(active_threshold) = msg.active_threshold.as_ref() {
        // Only check active threshold percentage as new tokens don't exist yet
        // We will check Absolute count (if configured) later for both existing
        // and new tokens.
        if let ActiveThreshold::Percentage { percent } = active_threshold {
            if *percent > Decimal::percent(100) || *percent <= Decimal::percent(0) {
                return Err(ContractError::InvalidActivePercentage {});
            }
        }
        ACTIVE_THRESHOLD.save(deps.storage, active_threshold)?;
    }

    // Save new token info for use in reply
    TOKEN_INSTANTIATION_INFO.save(deps.storage, &msg.token_info)?;

    match msg.token_info {
        TokenInfo::Existing { denom } => {
            // Validate active threshold absolute count if configured
            if let Some(ActiveThreshold::AbsoluteCount { count }) = msg.active_threshold {
                assert_valid_absolute_count_threshold(deps.as_ref(), &denom, count)?;
            }

            DENOM.save(deps.storage, &denom)?;

            // Instantiate cw-token-factory-issuer contract
            // DAO (sender) is set as admin
            let issuer_instantiate_msg = SubMsg::reply_on_success(
                WasmMsg::Instantiate {
                    admin: Some(info.sender.to_string()),
                    code_id: msg.token_issuer_code_id,
                    msg: to_binary(&IssuerInstantiateMsg::ExistingToken {
                        denom: denom.clone(),
                    })?,
                    funds: info.funds,
                    label: "cw-tokenfactory-issuer".to_string(),
                },
                INSTANTIATE_TOKEN_FACTORY_ISSUER_REPLY_ID,
            );

            Ok(Response::new()
                .add_attribute("action", "instantiate")
                .add_attribute("token", "existing_token")
                .add_attribute("denom", denom)
                .add_submessage(issuer_instantiate_msg))
        }
        TokenInfo::New(token) => {
            // Tnstantiate cw-token-factory-issuer contract
            // DAO (sender) is set as contract admin
            let issuer_instantiate_msg = SubMsg::reply_on_success(
                WasmMsg::Instantiate {
                    admin: Some(info.sender.to_string()),
                    code_id: msg.token_issuer_code_id,
                    msg: to_binary(&IssuerInstantiateMsg::NewToken {
                        subdenom: token.subdenom,
                    })?,
                    funds: info.funds,
                    label: "cw-tokenfactory-issuer".to_string(),
                },
                INSTANTIATE_TOKEN_FACTORY_ISSUER_REPLY_ID,
            );

            Ok(Response::new()
                .add_attribute("action", "instantiate")
                .add_attribute("token", "new_token")
                .add_submessage(issuer_instantiate_msg))
        }
    }
}

pub fn assert_valid_absolute_count_threshold(
    deps: Deps,
    token_denom: &str,
    count: Uint128,
) -> Result<(), ContractError> {
    if count.is_zero() {
        return Err(ContractError::ZeroActiveCount {});
    }
    let supply: Coin = deps.querier.query_supply(token_denom.to_string())?;
    if count > supply.amount {
        return Err(ContractError::InvalidAbsoluteCount {});
    }
    Ok(())
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
        ExecuteMsg::UpdateConfig { duration } => execute_update_config(deps, info, duration),
        ExecuteMsg::Claim {} => execute_claim(deps, env, info),
        ExecuteMsg::UpdateActiveThreshold { new_threshold } => {
            execute_update_active_threshold(deps, env, info, new_threshold)
        }
        ExecuteMsg::AddHook { addr } => execute_add_hook(deps, env, info, addr),
        ExecuteMsg::RemoveHook { addr } => execute_remove_hook(deps, env, info, addr),
    }
}

pub fn execute_stake(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let denom = DENOM.load(deps.storage)?;
    let amount = must_pay(&info, &denom)?;

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

    // Add stake hook messages
    let hook_msgs = stake_hook_msgs(HOOKS, deps.storage, info.sender.clone(), amount)?;

    Ok(Response::new()
        .add_submessages(hook_msgs)
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
    if amount.is_zero() {
        return Err(ContractError::ZeroUnstake {});
    }

    STAKED_BALANCES.update(
        deps.storage,
        &info.sender,
        env.block.height,
        |balance| -> Result<Uint128, ContractError> {
            balance
                .unwrap_or_default()
                .checked_sub(amount)
                .map_err(|_e| ContractError::InvalidUnstakeAmount {})
        },
    )?;
    STAKED_TOTAL.update(
        deps.storage,
        env.block.height,
        |total| -> Result<Uint128, ContractError> {
            total
                .unwrap_or_default()
                .checked_sub(amount)
                .map_err(|_e| ContractError::InvalidUnstakeAmount {})
        },
    )?;

    // Add unstake hook messages
    let hook_msgs = unstake_hook_msgs(HOOKS, deps.storage, info.sender.clone(), amount)?;

    let config = CONFIG.load(deps.storage)?;
    let denom = DENOM.load(deps.storage)?;
    match config.unstaking_duration {
        None => {
            let msg = CosmosMsg::Bank(BankMsg::Send {
                to_address: info.sender.to_string(),
                amount: coins(amount.u128(), denom),
            });
            Ok(Response::new()
                .add_message(msg)
                .add_submessages(hook_msgs)
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
                .add_submessages(hook_msgs)
                .add_attribute("action", "unstake")
                .add_attribute("from", info.sender)
                .add_attribute("amount", amount)
                .add_attribute("claim_duration", format!("{duration}")))
        }
    }
}

pub fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    duration: Option<Duration>,
) -> Result<Response, ContractError> {
    let mut config: Config = CONFIG.load(deps.storage)?;

    // Only the DAO can update the config
    let dao = DAO.load(deps.storage)?;
    if info.sender != dao {
        return Err(ContractError::Unauthorized {});
    }

    validate_duration(duration)?;

    config.unstaking_duration = duration;

    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new().add_attribute("action", "update_config"))
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

    let denom = DENOM.load(deps.storage)?;
    let msg = CosmosMsg::Bank(BankMsg::Send {
        to_address: info.sender.to_string(),
        amount: coins(release.u128(), denom),
    });

    Ok(Response::new()
        .add_message(msg)
        .add_attribute("action", "claim")
        .add_attribute("from", info.sender)
        .add_attribute("amount", release))
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

    if let Some(active_threshold) = new_active_threshold {
        match active_threshold {
            ActiveThreshold::Percentage { percent } => {
                if percent > Decimal::percent(100) || percent.is_zero() {
                    return Err(ContractError::InvalidActivePercentage {});
                }
            }
            ActiveThreshold::AbsoluteCount { count } => {
                let denom = DENOM.load(deps.storage)?;
                assert_valid_absolute_count_threshold(deps.as_ref(), &denom, count)?;
            }
        }
        ACTIVE_THRESHOLD.save(deps.storage, &active_threshold)?;
    } else {
        ACTIVE_THRESHOLD.remove(deps.storage);
    }

    Ok(Response::new().add_attribute("action", "update_active_threshold"))
}

pub fn execute_add_hook(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    addr: String,
) -> Result<Response, ContractError> {
    let dao = DAO.load(deps.storage)?;
    if info.sender != dao {
        return Err(ContractError::Unauthorized {});
    }

    let hook = deps.api.addr_validate(&addr)?;
    HOOKS.add_hook(deps.storage, hook)?;
    Ok(Response::new()
        .add_attribute("action", "add_hook")
        .add_attribute("hook", addr))
}

pub fn execute_remove_hook(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    addr: String,
) -> Result<Response, ContractError> {
    let dao = DAO.load(deps.storage)?;
    if info.sender != dao {
        return Err(ContractError::Unauthorized {});
    }

    let hook = deps.api.addr_validate(&addr)?;
    HOOKS.remove_hook(deps.storage, hook)?;
    Ok(Response::new()
        .add_attribute("action", "remove_hook")
        .add_attribute("hook", addr))
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
        QueryMsg::GetConfig {} => to_binary(&CONFIG.load(deps.storage)?),
        QueryMsg::Denom {} => to_binary(&DenomResponse {
            denom: DENOM.load(deps.storage)?,
        }),
        QueryMsg::ListStakers { start_after, limit } => {
            query_list_stakers(deps, start_after, limit)
        }
        QueryMsg::IsActive {} => query_is_active(deps),
        QueryMsg::ActiveThreshold {} => query_active_threshold(deps),
        QueryMsg::GetHooks {} => to_binary(&query_hooks(deps)?),
        QueryMsg::TokenContract {} => to_binary(&TOKEN_ISSUER_CONTRACT.load(deps.storage)?),
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
    to_binary(&dao_interface::voting::InfoResponse { info })
}

pub fn query_dao(deps: Deps) -> StdResult<Binary> {
    let dao = DAO.load(deps.storage)?;
    to_binary(&dao)
}

pub fn query_claims(deps: Deps, address: String) -> StdResult<ClaimsResponse> {
    CLAIMS.query_claims(deps, &deps.api.addr_validate(&address)?)
}

pub fn query_list_stakers(
    deps: Deps,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<Binary> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let addr = maybe_addr(deps.api, start_after)?;
    let start = addr.as_ref().map(Bound::exclusive);

    let stakers = STAKED_BALANCES
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|item| {
            item.map(|(address, balance)| StakerBalanceResponse {
                address: address.into_string(),
                balance,
            })
        })
        .collect::<StdResult<_>>()?;

    to_binary(&ListStakersResponse { stakers })
}

pub fn query_is_active(deps: Deps) -> StdResult<Binary> {
    let threshold = ACTIVE_THRESHOLD.may_load(deps.storage)?;
    if let Some(threshold) = threshold {
        let denom = DENOM.load(deps.storage)?;
        let actual_power = STAKED_TOTAL.may_load(deps.storage)?.unwrap_or_default();
        match threshold {
            ActiveThreshold::AbsoluteCount { count } => to_binary(&IsActiveResponse {
                active: actual_power >= count,
            }),
            ActiveThreshold::Percentage { percent } => {
                // percent is bounded between [0, 100]. decimal
                // represents percents in u128 terms as p *
                // 10^15. this bounds percent between [0, 10^17].
                //
                // total_potential_power is bounded between [0, 2^128]
                // as it tracks the balances of a cw20 token which has
                // a max supply of 2^128.
                //
                // with our precision factor being 10^9:
                //
                // total_power <= 2^128 * 10^9 <= 2^256
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
                let total_potential_power: cosmwasm_std::SupplyResponse =
                    deps.querier
                        .query(&cosmwasm_std::QueryRequest::Bank(BankQuery::Supply {
                            denom,
                        }))?;
                let total_power = total_potential_power
                    .amount
                    .amount
                    .full_mul(PRECISION_FACTOR);
                // under the hood decimals are `atomics / 10^decimal_places`.
                // cosmwasm doesn't give us a Decimal * Uint256
                // implementation so we take the decimal apart and
                // multiply by the fraction.
                let applied = total_power.multiply_ratio(
                    percent.atomics(),
                    Uint256::from(10u64).pow(percent.decimal_places()),
                );
                let rounded = (applied + Uint256::from(PRECISION_FACTOR) - Uint256::from(1u128))
                    / Uint256::from(PRECISION_FACTOR);
                let count: Uint128 = rounded.try_into().unwrap();
                to_binary(&IsActiveResponse {
                    active: actual_power >= count,
                })
            }
        }
    } else {
        to_binary(&IsActiveResponse { active: true })
    }
}

pub fn query_active_threshold(deps: Deps) -> StdResult<Binary> {
    to_binary(&ActiveThresholdResponse {
        active_threshold: ACTIVE_THRESHOLD.may_load(deps.storage)?,
    })
}

pub fn query_hooks(deps: Deps) -> StdResult<GetHooksResponse> {
    Ok(GetHooksResponse {
        hooks: HOOKS.query_hooks(deps)?.hooks,
    })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    let storage_version: ContractVersion = get_contract_version(deps.storage)?;

    // Only migrate if newer
    if storage_version.version < CONTRACT_VERSION.to_string() {
        // Set contract to version to latest
        set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    }

    Ok(Response::new().add_attribute("action", "migrate"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> Result<Response, ContractError> {
    match msg.id {
        INSTANTIATE_TOKEN_FACTORY_ISSUER_REPLY_ID => {
            // Parse and save address of cw-tokenfactory-issuer
            let issuer_addr = parse_reply_instantiate_data(msg)?.contract_address;
            TOKEN_ISSUER_CONTRACT.save(deps.storage, &deps.api.addr_validate(&issuer_addr)?)?;

            // Load info for new token and remove temporary data
            let token_info = TOKEN_INSTANTIATION_INFO.load(deps.storage)?;
            TOKEN_INSTANTIATION_INFO.remove(deps.storage);

            match token_info {
                TokenInfo::Existing { .. } => {
                    // Not much to do here.
                    Ok(
                        Response::new()
                            .add_attribute("cw-tokenfactory-issuer-address", issuer_addr),
                    )
                }
                TokenInfo::New(token) => {
                    // Load the DAO address
                    let dao = DAO.load(deps.storage)?;

                    // Format the denom and save it
                    let denom = format!("factory/{}/{}", &issuer_addr, token.subdenom);

                    DENOM.save(deps.storage, &denom)?;

                    // Check supply is greater than zero, iterate through initial
                    // balances and sum them, add DAO balance as well.
                    let initial_supply = token
                        .initial_balances
                        .iter()
                        .fold(Uint128::zero(), |previous, new_balance| {
                            previous + new_balance.amount
                        });
                    let total_supply =
                        initial_supply + token.initial_dao_balance.unwrap_or_default();

                    // Validate active threshold absolute count if configured
                    if let Some(ActiveThreshold::AbsoluteCount { count }) =
                        ACTIVE_THRESHOLD.may_load(deps.storage)?
                    {
                        if count.is_zero() {
                            return Err(ContractError::ZeroActiveCount {});
                        }
                        if count > initial_supply {
                            return Err(ContractError::InvalidAbsoluteCount {});
                        }
                    }

                    // Cannot instantiate with no initial token owners because it would
                    // immediately lock the DAO.
                    if initial_supply.is_zero() {
                        return Err(ContractError::InitialBalancesError {});
                    }

                    // Msgs to be executed to finalize setup
                    let mut msgs: Vec<WasmMsg> = vec![];

                    // Grant an allowance to mint the initial supply
                    msgs.push(WasmMsg::Execute {
                        contract_addr: issuer_addr.clone(),
                        msg: to_binary(&IssuerExecuteMsg::SetMinterAllowance {
                            address: env.contract.address.to_string(),
                            allowance: total_supply,
                        })?,
                        funds: vec![],
                    });

                    // If metadata, set it by calling the contract
                    if let Some(metadata) = token.metadata {
                        // The first denom_unit must be the same as the tf and base denom.
                        // It must have an exponent of 0. This the smallest unit of the token.
                        // For more info: // https://docs.cosmos.network/main/architecture/adr-024-coin-metadata
                        let mut denom_units = vec![DenomUnit {
                            denom: denom.clone(),
                            exponent: 0,
                            aliases: vec![token.subdenom],
                        }];

                        // Caller can optionally define additional units
                        if let Some(mut additional_units) = metadata.additional_denom_units {
                            denom_units.append(&mut additional_units);
                        }

                        // Sort denom units by exponent, must be in ascending order
                        denom_units.sort_by(|a, b| a.exponent.cmp(&b.exponent));

                        msgs.push(WasmMsg::Execute {
                            contract_addr: issuer_addr.clone(),
                            msg: to_binary(&IssuerExecuteMsg::SetDenomMetadata {
                                metadata: Metadata {
                                    description: metadata.description,
                                    denom_units,
                                    base: denom.clone(),
                                    display: metadata.display,
                                    name: metadata.name,
                                    symbol: metadata.symbol,
                                },
                            })?,
                            funds: vec![],
                        });
                    }

                    // Call issuer contract to mint tokens for initial balances
                    token
                        .initial_balances
                        .iter()
                        .for_each(|b: &InitialBalance| {
                            msgs.push(WasmMsg::Execute {
                                contract_addr: issuer_addr.clone(),
                                msg: to_binary(&IssuerExecuteMsg::Mint {
                                    to_address: b.address.clone(),
                                    amount: b.amount,
                                })
                                .unwrap_or_default(),
                                funds: vec![],
                            });
                        });

                    // Add initial DAO balance to initial_balances if nonzero.
                    if let Some(initial_dao_balance) = token.initial_dao_balance {
                        if !initial_dao_balance.is_zero() {
                            msgs.push(WasmMsg::Execute {
                                contract_addr: issuer_addr.clone(),
                                msg: to_binary(&IssuerExecuteMsg::Mint {
                                    to_address: dao.to_string(),
                                    amount: initial_dao_balance,
                                })?,
                                funds: vec![],
                            });
                        }
                    }

                    // Update issuer contract owner to be the DAO
                    msgs.push(WasmMsg::Execute {
                        contract_addr: issuer_addr.clone(),
                        msg: to_binary(&IssuerExecuteMsg::UpdateContractOwner {
                            new_owner: dao.to_string(),
                        })?,
                        funds: vec![],
                    });

                    Ok(Response::new()
                        .add_attribute("cw-tokenfactory-issuer-address", issuer_addr)
                        .add_attribute("denom", denom)
                        .add_messages(msgs))
                }
            }
        }
        _ => Err(ContractError::UnknownReplyId { id: msg.id }),
    }
}
