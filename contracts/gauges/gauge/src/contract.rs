#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    ensure, to_json_binary, Addr, Binary, Decimal, Deps, DepsMut, Env, MessageInfo, Order,
    QueryRequest, Response, StdError, StdResult, Uint128, WasmMsg, WasmQuery,
};
use cw2::{ensure_from_older_version, set_contract_version};
use cw_storage_plus::Bound;
use dao_interface::{
    msg::ExecuteMsg as DaoExecuteMsg,
    voting::{Query as DaoQuery, VotingPowerAtHeightResponse},
};
use execute::execute_update_owner;

use crate::msg::{
    AdapterQueryMsg, AllOptionsResponse, CheckOptionResponse, ExecuteMsg, GaugeConfig,
    GaugeResponse, InstantiateMsg, ListGaugesResponse, ListOptionsResponse, ListVotesResponse,
    MigrateMsg, QueryMsg, SampleGaugeMsgsResponse, SelectedSetResponse,
};
use crate::state::{
    fetch_last_id, update_tally, votes, Config, Gauge, GaugeId, CONFIG, GAUGES, OPTION_BY_POINTS,
    TALLY, TOTAL_CAST,
};
use crate::{error::ContractError, state::Reset};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:gauge";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    cw_ownable::initialize_owner(deps.storage, deps.api, Some(&msg.owner))?;

    let voting_powers = deps.api.addr_validate(&msg.voting_powers)?;
    let hook_caller = deps.api.addr_validate(&msg.hook_caller)?;
    let config = Config {
        voting_powers,
        hook_caller,
        dao_core: info.sender,
    };
    CONFIG.save(deps.storage, &config)?;

    for gauge in msg.gauges.unwrap_or_default() {
        execute::attach_gauge(deps.branch(), env.clone(), gauge)?;
    }

    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("owner", &msg.owner)
        .add_attribute("voting_powers", &msg.voting_powers))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::StakeChangeHook(msg) => execute::stake_changed(deps, info, msg),
        ExecuteMsg::NftStakeChangeHook(msg) => execute::nft_stake_changed(deps, info, msg),
        ExecuteMsg::MemberChangedHook(hook_msg) => {
            execute::member_changed(deps, info.sender, hook_msg.diffs)
        }
        ExecuteMsg::CreateGauge(options) => execute::create_gauge(deps, env, info.sender, options),
        ExecuteMsg::UpdateGauge {
            gauge_id,
            epoch_limit,
            epoch_size,
            min_percent_selected,
            max_options_selected,
            max_available_percentage,
        } => execute::update_gauge(
            deps,
            info.sender,
            gauge_id,
            epoch_size,
            epoch_limit,
            min_percent_selected,
            max_options_selected,
            max_available_percentage,
        ),
        ExecuteMsg::StopGauge { gauge } => execute::stop_gauge(deps, info.sender, gauge),
        ExecuteMsg::ResetGauge { gauge, batch_size } => {
            execute::reset_gauge(deps, env, gauge, batch_size)
        }
        ExecuteMsg::AddOption { gauge, option } => {
            execute::add_option(deps, info.sender, gauge, option, true)
        }
        ExecuteMsg::RemoveOption { gauge, option } => {
            execute::remove_option(deps, info.sender, gauge, option)
        }
        ExecuteMsg::PlaceVotes { gauge, votes } => {
            execute::place_votes(deps, env, info.sender, gauge, votes)
        }
        ExecuteMsg::Execute { gauge } => execute::execute(deps, env, gauge),
        ExecuteMsg::UpdateOwnership(action) => execute_update_owner(deps, info, env, action),
    }
}

mod execute {
    use cosmwasm_std::CosmosMsg;
    use cw4::MemberDiff;
    use cw_utils::nonpayable;
    use dao_hooks::{nft_stake::NftStakeChangedHookMsg, stake::StakeChangedHookMsg};

    use super::*;
    use crate::{
        msg::CreateGaugeReply,
        state::{remove_tally, update_tallies, Reset, Vote},
    };
    use std::collections::HashMap;

    pub fn member_changed(
        deps: DepsMut,
        sender: Addr,
        diffs: Vec<MemberDiff>,
    ) -> Result<Response, ContractError> {
        // make sure only hook caller contract can activate this endpoint
        if sender != CONFIG.load(deps.storage)?.hook_caller {
            return Err(ContractError::Unauthorized {});
        }

        let mut response = Response::new().add_attribute("action", "member_changed_hook");
        let mut gauges = HashMap::new();

        for diff in diffs {
            response = response.add_attribute("member", &diff.key);
            let voter = deps.api.addr_validate(&diff.key)?;

            // for each gauge this user voted on,
            // update the tallies and update the users vote power
            for mut vote in
                votes().query_votes_by_voter(deps.as_ref(), &voter, None, Some(query::MAX_LIMIT))?
            {
                // find change of vote powers
                let old = Uint128::new(diff.old.unwrap_or_default().into());
                let new = Uint128::new(diff.new.unwrap_or_default().into());

                // load gauge if not already loaded
                let gauge = gauges
                    .entry(vote.gauge_id)
                    .or_insert_with(|| GAUGES.load(deps.storage, vote.gauge_id).unwrap());

                if vote.is_expired(gauge) {
                    continue;
                }

                // calculate updates and adjust tallies
                let updates: Vec<_> = vote
                    .votes
                    .iter()
                    .map(|v| {
                        (
                            v.option.as_str(),
                            (old * v.weight).u128(),
                            (new * v.weight).u128(),
                        )
                    })
                    .collect();
                update_tallies(deps.storage, vote.gauge_id, updates)?;

                // store new vote power for this user
                vote.power = new;
                votes().save(deps.storage, &voter, vote.gauge_id, &vote)?;
            }
        }

        Ok(response)
    }

    pub fn stake_changed(
        deps: DepsMut,
        info: MessageInfo,
        msg: StakeChangedHookMsg,
    ) -> Result<Response, ContractError> {
        // make sure only hook caller contract can activate this endpoint
        if info.sender != CONFIG.load(deps.storage)?.hook_caller {
            return Err(ContractError::Unauthorized {});
        }

        match msg {
            StakeChangedHookMsg::Stake { addr, amount } => {
                // for each gauge this user voted on,
                // update the tallies and update the users vote power
                for mut vote in votes().query_votes_by_voter(
                    deps.as_ref(),
                    &addr,
                    None,
                    Some(query::MAX_LIMIT),
                )? {
                    let gauge = GAUGES.load(deps.storage, vote.gauge_id)?;

                    let old = vote.power;

                    // Voting power increases with staking amount
                    let new = vote.power + amount;

                    if vote.is_expired(&gauge) {
                        continue;
                    }

                    // calculate updates and adjust tallies
                    let updates: Vec<_> = vote
                        .votes
                        .iter()
                        .map(|v| {
                            (
                                v.option.as_str(),
                                (old * v.weight).u128(),
                                (new * v.weight).u128(),
                            )
                        })
                        .collect();
                    update_tallies(deps.storage, vote.gauge_id, updates)?;

                    // Update and store new vote power for this user
                    vote.power = new;
                    votes().save(deps.storage, &addr, vote.gauge_id, &vote)?;
                }

                Ok(Response::new())
            }
            StakeChangedHookMsg::Unstake { addr, amount } => {
                // for each gauge this user voted on,
                // update the tallies and update the users vote power
                for mut vote in votes().query_votes_by_voter(
                    deps.as_ref(),
                    &addr,
                    None,
                    Some(query::MAX_LIMIT),
                )? {
                    let gauge = GAUGES.load(deps.storage, vote.gauge_id)?;

                    let old = vote.power;

                    // Decrease voting power by unstaked amount
                    let new = vote.power - amount;

                    if vote.is_expired(&gauge) {
                        continue;
                    }

                    // calculate updates and adjust tallies
                    let updates: Vec<_> = vote
                        .votes
                        .iter()
                        .map(|v| {
                            (
                                v.option.as_str(),
                                (old * v.weight).u128(),
                                (new * v.weight).u128(),
                            )
                        })
                        .collect();
                    update_tallies(deps.storage, vote.gauge_id, updates)?;

                    // Update and store new vote power for this user
                    vote.power = new;
                    votes().save(deps.storage, &addr, vote.gauge_id, &vote)?;
                }

                Ok(Response::new())
            }
        }
    }

    pub fn nft_stake_changed(
        deps: DepsMut,
        info: MessageInfo,
        msg: NftStakeChangedHookMsg,
    ) -> Result<Response, ContractError> {
        // make sure only hook caller contract can activate this endpoint
        if info.sender != CONFIG.load(deps.storage)?.hook_caller {
            return Err(ContractError::Unauthorized {});
        }

        match msg {
            NftStakeChangedHookMsg::Stake { addr, token_id: _ } => {
                // for each gauge this user voted on,
                // update the tallies and update the users vote power
                for mut vote in votes().query_votes_by_voter(
                    deps.as_ref(),
                    &addr,
                    None,
                    Some(query::MAX_LIMIT),
                )? {
                    let gauge = GAUGES.load(deps.storage, vote.gauge_id)?;

                    let old = vote.power;
                    // Voting power increases by one (only one token_id staked at a time)
                    let new = vote.power + Uint128::one();

                    if vote.is_expired(&gauge) {
                        continue;
                    }

                    // calculate updates and adjust tallies
                    let updates: Vec<_> = vote
                        .votes
                        .iter()
                        .map(|v| {
                            (
                                v.option.as_str(),
                                (old * v.weight).u128(),
                                (new * v.weight).u128(),
                            )
                        })
                        .collect();
                    update_tallies(deps.storage, vote.gauge_id, updates)?;

                    // Update and store new vote power for this user
                    vote.power = new;
                    votes().save(deps.storage, &addr, vote.gauge_id, &vote)?;
                }

                Ok(Response::new())
            }
            NftStakeChangedHookMsg::Unstake { addr, token_ids } => {
                // for each gauge this user voted on,
                // update the tallies and update the users vote power
                for mut vote in votes().query_votes_by_voter(
                    deps.as_ref(),
                    &addr,
                    None,
                    Some(query::MAX_LIMIT),
                )? {
                    let gauge = GAUGES.load(deps.storage, vote.gauge_id)?;

                    let old = vote.power;

                    // Decrease voting power by number of token_ids
                    let amount: u128 = token_ids.len().try_into().unwrap();
                    let new = vote.power - Uint128::new(amount);

                    if vote.is_expired(&gauge) {
                        continue;
                    }

                    // calculate updates and adjust tallies
                    let updates: Vec<_> = vote
                        .votes
                        .iter()
                        .map(|v| {
                            (
                                v.option.as_str(),
                                (old * v.weight).u128(),
                                (new * v.weight).u128(),
                            )
                        })
                        .collect();
                    update_tallies(deps.storage, vote.gauge_id, updates)?;

                    // Update and store new vote power for this user
                    vote.power = new;
                    votes().save(deps.storage, &addr, vote.gauge_id, &vote)?;
                }

                Ok(Response::new())
            }
        }
    }

    pub fn create_gauge(
        deps: DepsMut,
        env: Env,
        sender: Addr,
        options: GaugeConfig,
    ) -> Result<Response, ContractError> {
        cw_ownable::assert_owner(deps.storage, &sender)?;

        let adapter = attach_gauge(deps, env, options)?;

        Ok(Response::new()
            .add_attribute("action", "create_gauge")
            .add_attribute("gauge-id", adapter.id.to_string())
            .add_attribute("adapter", adapter.addr))
    }

    pub fn attach_gauge(
        mut deps: DepsMut,
        env: Env,
        GaugeConfig {
            title,
            adapter,
            epoch_size,
            min_percent_selected,
            max_options_selected,
            max_available_percentage,
            reset_epoch,
            total_epochs,
        }: GaugeConfig,
    ) -> Result<CreateGaugeReply, ContractError> {
        let adapter = deps.api.addr_validate(&adapter)?;
        // gauge parameter validation
        ensure!(epoch_size > 60u64, ContractError::EpochSizeTooShort {});
        if let Some(min_percent_selected) = min_percent_selected {
            ensure!(
                min_percent_selected < Decimal::one(),
                ContractError::MinPercentSelectedTooBig {}
            );
        }
        ensure!(
            max_options_selected > 0,
            ContractError::MaxOptionsSelectedTooSmall {}
        );
        let gauge = Gauge {
            title,
            adapter: adapter.clone(),
            epoch: epoch_size,
            count: Some(0),
            min_percent_selected,
            max_options_selected,
            max_available_percentage,
            is_stopped: false,
            next_epoch: env.block.time.seconds() + epoch_size,
            last_executed_set: None,
            reset: reset_epoch.map(|r| Reset {
                last: None,
                reset_each: r,
                next: env.block.time.plus_seconds(r).seconds(),
            }),
            total_epoch: total_epochs,
        };
        let last_id: GaugeId = fetch_last_id(deps.storage)?;
        GAUGES.save(deps.storage, last_id, &gauge)?;

        // fetch adapter options
        let adapter_options: AllOptionsResponse =
            deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                contract_addr: adapter.to_string(),
                msg: to_json_binary(&AdapterQueryMsg::AllOptions {})?,
            }))?;
        adapter_options.options.into_iter().try_for_each(|option| {
            execute::add_option(deps.branch(), adapter.clone(), last_id, option, false)?;
            Ok::<_, ContractError>(())
        })?;

        Ok(CreateGaugeReply {
            id: last_id,
            addr: adapter.to_string(),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_gauge(
        deps: DepsMut,
        sender: Addr,
        gauge_id: u64,
        epoch_limit: Option<u64>,
        epoch_size: Option<u64>,
        min_percent_selected: Option<Decimal>,
        max_options_selected: Option<u32>,
        max_available_percentage: Option<Decimal>,
    ) -> Result<Response, ContractError> {
        cw_ownable::assert_owner(deps.storage, &sender)?;

        let mut gauge = GAUGES.load(deps.storage, gauge_id)?;
        // updated epoch size must be greater than 60 seconds
        if let Some(epoch_size) = epoch_size {
            ensure!(epoch_size > 60u64, ContractError::EpochSizeTooShort {});
            gauge.epoch = epoch_size;
        }
        // updated epoch limit count must not already have passed
        if let Some(epoch_limit) = epoch_limit {
            let e = gauge.gauge_epoch()?;
            ensure!(e < epoch_limit, ContractError::EpochLimitTooShort {})
        }
        // min_percent_selected percent must be less than 100%. None if 0.
        if let Some(min_percent_selected) = min_percent_selected {
            if min_percent_selected.is_zero() {
                gauge.min_percent_selected = None
            } else {
                ensure!(
                    min_percent_selected < Decimal::one(),
                    ContractError::MinPercentSelectedTooBig {}
                );
                gauge.min_percent_selected = Some(min_percent_selected)
            };
        }
        // max_options_selected must be at least 1
        if let Some(max_options_selected) = max_options_selected {
            ensure!(
                max_options_selected > 0,
                ContractError::MaxOptionsSelectedTooSmall {}
            );
            gauge.max_options_selected = max_options_selected;
        }
        // max_available_percentage must be less than 100%. None if 0.
        if let Some(max_available_percentage) = max_available_percentage {
            if max_available_percentage.is_zero() {
                gauge.max_available_percentage = None
            } else {
                ensure!(
                    max_available_percentage < Decimal::one(),
                    ContractError::MaxAvailablePercentTooBig {}
                );
                gauge.max_available_percentage = Some(max_available_percentage)
            };
        }
        GAUGES.save(deps.storage, gauge_id, &gauge)?;

        Ok(Response::new().add_attribute("action", "update_gauge"))
    }

    pub fn stop_gauge(
        deps: DepsMut,
        sender: Addr,
        gauge_id: GaugeId,
    ) -> Result<Response, ContractError> {
        cw_ownable::assert_owner(deps.storage, &sender)?;

        let gauge = GAUGES.load(deps.storage, gauge_id)?;
        let gauge = Gauge {
            is_stopped: true,
            ..gauge
        };
        GAUGES.save(deps.storage, gauge_id, &gauge)?;

        Ok(Response::new()
            .add_attribute("action", "stop_gauge")
            .add_attribute("gauge_id", gauge_id.to_string()))
    }

    pub fn remove_option(
        deps: DepsMut,
        sender: Addr,
        gauge_id: GaugeId,
        option: String,
    ) -> Result<Response, ContractError> {
        // check if such option even exists
        if !TALLY.has(deps.as_ref().storage, (gauge_id, &option)) {
            return Err(ContractError::OptionDoesNotExists { option, gauge_id });
        };

        // only owner can remove option for now
        cw_ownable::assert_owner(deps.storage, &sender)?;
        remove_tally(deps.storage, gauge_id, &option)?;

        Ok(Response::new()
            .add_attribute("action", "remove_option")
            .add_attribute("sender", &sender)
            .add_attribute("gauge_id", gauge_id.to_string())
            .add_attribute("option", option))
    }

    pub fn reset_gauge(
        deps: DepsMut,
        env: Env,
        gauge_id: GaugeId,
        batch_size: u32,
    ) -> Result<Response, ContractError> {
        let mut gauge = GAUGES.load(deps.storage, gauge_id)?;
        match gauge.reset {
            Some(ref mut reset) if reset.next <= env.block.time.seconds() => {
                reset.last = Some(reset.next);

                // remove all options from the gauge
                let keys = OPTION_BY_POINTS
                    .sub_prefix(gauge_id)
                    .keys(deps.storage, None, None, Order::Ascending)
                    .take(batch_size as usize)
                    .collect::<StdResult<Vec<_>>>()?;
                for (points, option) in &keys {
                    OPTION_BY_POINTS.remove(deps.storage, (gauge_id, *points, option));
                    OPTION_BY_POINTS.save(deps.storage, (gauge_id, 0, option), &1)?;
                    TALLY.save(deps.storage, (gauge_id, option), &0)?;
                }

                // if this is the last batch, update the reset epoch
                if (keys.len() as u32) < batch_size {
                    // removing total cast only once at the end to save gas
                    TOTAL_CAST.save(deps.storage, gauge_id, &0)?;
                    reset.next += reset.reset_each;
                }
            }
            Some(_) => {
                return Err(ContractError::ResetEpochNotPassed {});
            }
            None => {
                return Err(ContractError::Unauthorized {});
            }
        }

        GAUGES.save(deps.storage, gauge_id, &gauge)?;

        Ok(Response::new()
            .add_attribute("action", "reset_gauge")
            .add_attribute("gauge_id", gauge_id.to_string()))
    }

    // TODO this doesn't seem very safe... double check permissions here
    // Why is check option optional?
    pub fn add_option(
        deps: DepsMut,
        sender: Addr,
        gauge_id: GaugeId,
        option: String,
        // must be true if option is added by execute message
        check_option: bool,
    ) -> Result<Response, ContractError> {
        // check is such option already exists
        if TALLY.has(deps.as_ref().storage, (gauge_id, &option)) {
            return Err(ContractError::OptionAlreadyExists { option, gauge_id });
        };

        // only options added from gauge creation level should not be validated and can
        // have 0 points as assigned voting power.
        if check_option {
            let gauge = GAUGES.load(deps.storage, gauge_id)?;
            // query gauge adapter if it is valid
            let adapter_option: CheckOptionResponse = deps
                .querier
                .query_wasm_smart(
                    gauge.adapter,
                    &AdapterQueryMsg::CheckOption {
                        option: option.clone(),
                    },
                )
                .map_err(|_| ContractError::OptionInvalidByAdapter {
                    option: option.clone(),
                    gauge_id,
                })?;
            if !adapter_option.valid {
                return Err(ContractError::OptionInvalidByAdapter { option, gauge_id });
            }
            // If it is a user adding option, query him for voting power in order to prevent
            // spam from nonvoting users
            let voting_power = deps
                .querier
                .query::<VotingPowerAtHeightResponse>(&QueryRequest::Wasm(WasmQuery::Smart {
                    contract_addr: CONFIG.load(deps.storage)?.voting_powers.to_string(),
                    msg: to_json_binary(&DaoQuery::VotingPowerAtHeight {
                        address: sender.to_string(),
                        height: None,
                    })?,
                }))?
                .power;
            if voting_power.is_zero() {
                return Err(ContractError::NoVotingPower(sender.to_string()));
            }
        }

        update_tally(deps.storage, gauge_id, &option, 0u128, 0u128)?;

        Ok(Response::new()
            .add_attribute("action", "add_option")
            .add_attribute("sender", &sender)
            .add_attribute("gauge_id", gauge_id.to_string())
            .add_attribute("option", option))
    }

    pub fn place_votes(
        deps: DepsMut,
        env: Env,
        sender: Addr,
        gauge_id: GaugeId,
        new_votes: Option<Vec<Vote>>,
    ) -> Result<Response, ContractError> {
        let gauge = match GAUGES.may_load(deps.storage, gauge_id)? {
            Some(gauge) => gauge,
            None => return Err(ContractError::GaugeMissing(gauge_id)),
        };

        if gauge.is_resetting() {
            return Err(ContractError::GaugeResetting(gauge_id));
        }

        // make sure sums work out
        let new_votes = new_votes.unwrap_or_default();
        let total_weight = new_votes.iter().map(|v| v.weight).sum();
        if total_weight > Decimal::one() {
            return Err(ContractError::TooMuchVotingWeight(total_weight));
        }

        // load voter power from voting powers contract (DAO)
        let voting_power = deps
            .querier
            .query::<VotingPowerAtHeightResponse>(&QueryRequest::Wasm(WasmQuery::Smart {
                contract_addr: CONFIG.load(deps.storage)?.voting_powers.to_string(),
                msg: to_json_binary(&DaoQuery::VotingPowerAtHeight {
                    address: sender.to_string(),
                    height: None,
                })?,
            }))?
            .power;
        if voting_power.is_zero() {
            return Err(ContractError::NoVotingPower(sender.to_string()));
        }

        let mut previous_vote = votes().may_load(deps.storage, &sender, gauge_id)?;
        if let Some(v) = &previous_vote {
            if v.is_expired(&gauge) {
                previous_vote = None;
            }
        }
        if previous_vote.is_none() && new_votes.is_empty() {
            return Err(ContractError::CannotRemoveNonexistingVote {});
        }

        // first, calculate a diff between new_vote and previous_vote (option -> (old, new))
        let previous_vote = previous_vote.unwrap_or_default();
        let power = previous_vote.power;
        let mut diff: HashMap<&str, (u128, u128)> = previous_vote
            .votes
            .iter()
            .map(|v| (v.option.as_str(), ((power * v.weight).u128(), 0u128)))
            .collect();
        for v in new_votes.iter() {
            let new = (voting_power * v.weight).u128();
            let add = match diff.remove(v.option.as_str()) {
                Some((old, _)) => (old, new),
                None => (0, new),
            };
            diff.insert(&v.option, add);
        }

        // second, test any new options are valid,
        // only for those voted for first time (others have already been checked)
        for new_opt in diff
            .iter()
            .filter(|(_, (old, _))| *old == 0)
            .map(|(&k, _)| k)
        {
            if !TALLY.has(deps.storage, (gauge_id, new_opt)) {
                return Err(ContractError::OptionDoesNotExists {
                    option: new_opt.to_string(),
                    gauge_id,
                });
            }
        }

        // third, update tally based on diff
        let updates: Vec<(&str, u128, u128)> = diff
            .iter()
            .map(|(&k, (old, new))| (k, *old, *new))
            .collect();
        update_tallies(deps.storage, gauge_id, updates)?;

        // finally, update the votes for this user
        if new_votes.is_empty() {
            // completely remove sender's votes
            votes().remove_votes(deps.storage, &sender, gauge_id)?;
        } else {
            // store sender's new votes (overwriting old votes)
            votes().set_votes(
                deps.storage,
                &env,
                &sender,
                gauge_id,
                new_votes,
                voting_power,
            )?;
        }

        let response = Response::new()
            .add_attribute("action", "place_vote")
            .add_attribute("sender", &sender)
            .add_attribute("gauge_id", gauge_id.to_string());
        Ok(response)
    }

    pub fn execute(deps: DepsMut, env: Env, gauge_id: u64) -> Result<Response, ContractError> {
        let mut gauge = GAUGES.load(deps.storage, gauge_id)?;
        let mut msgs = vec![];

        if gauge.is_stopped {
            return Err(ContractError::GaugeStopped(gauge_id));
        }
        if gauge.is_resetting() {
            return Err(ContractError::GaugeResetting(gauge_id));
        }

        let current_epoch = env.block.time.seconds();
        if current_epoch < gauge.next_epoch {
            return Err(ContractError::EpochNotReached {
                gauge_id,
                current_epoch,
                next_epoch: gauge.next_epoch,
            });
        }
        gauge.next_epoch = env.block.time.plus_seconds(gauge.epoch).seconds();

        // this set contains tuple (option, total_voted_power)
        // for adapter query, this needs to be transformed into (option, voted_weight)
        let selected_set_with_powers = query::selected_set(deps.as_ref(), gauge_id)?.votes;
        let selected_powers_sum = selected_set_with_powers
            .iter()
            .map(|(_, power)| power.u128())
            .sum::<u128>();

        // save the selected options and their powers for the frontend to display
        gauge.last_executed_set = Some(selected_set_with_powers.clone());

        // calculate "local" ratios of voted options per total power of all selected options
        let selected = selected_set_with_powers
            .into_iter()
            .map(|(option, power)| Ok((option, Decimal::from_ratio(power, selected_powers_sum))))
            .collect::<StdResult<Vec<(String, Decimal)>>>()?;

        // query gauge adapter for execute messages for DAO
        let execute_messages: SampleGaugeMsgsResponse = deps.querier.query_wasm_smart(
            gauge.adapter.clone(),
            &AdapterQueryMsg::SampleGaugeMsgs { selected },
        )?;

        // Adds msgs from query to response
        msgs.extend(execute_messages.execute);

        // increments epoch count
        gauge.count = gauge.increment_gauge_count()?;

        // Will add a stop_gauge msg to list of msgs for DAO if gauge config has total_epochs set
        if gauge.will_reach_epoch_limit() {
            msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: env.contract.address.to_string(),
                msg: to_json_binary(&ExecuteMsg::StopGauge { gauge: gauge_id })?,
                funds: vec![],
            }))
        }

        let config = CONFIG.load(deps.storage)?;
        let execute_msg = WasmMsg::Execute {
            contract_addr: config.dao_core.to_string(),
            msg: to_json_binary(&DaoExecuteMsg::ExecuteProposalHook { msgs })?,
            funds: vec![],
        };

        GAUGES.save(deps.storage, gauge_id, &gauge)?;

        Ok(Response::new()
            .add_attribute("action", "execute_tally")
            .add_message(execute_msg))
    }

    pub fn execute_update_owner(
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
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Info {} => Ok(to_json_binary(&query::info(deps)?)?),
        QueryMsg::Gauge { id } => Ok(to_json_binary(&query::gauge(deps, id)?)?),
        QueryMsg::ListGauges { start_after, limit } => Ok(to_json_binary(&query::list_gauges(
            deps,
            start_after,
            limit,
        )?)?),
        QueryMsg::Vote { gauge, voter } => Ok(to_json_binary(&query::vote(deps, gauge, voter)?)?),
        QueryMsg::ListVotes {
            gauge,
            start_after,
            limit,
        } => Ok(to_json_binary(&query::list_votes(
            deps,
            gauge,
            start_after,
            limit,
        )?)?),
        QueryMsg::ListOptions {
            gauge,
            start_after,
            limit,
        } => Ok(to_json_binary(&query::list_options(
            deps,
            gauge,
            start_after,
            limit,
        )?)?),
        QueryMsg::SelectedSet { gauge } => Ok(to_json_binary(&query::selected_set(deps, gauge)?)?),
        QueryMsg::LastExecutedSet { gauge } => {
            Ok(to_json_binary(&query::last_executed_set(deps, gauge)?)?)
        }
        QueryMsg::Ownership {} => to_json_binary(&cw_ownable::get_ownership(deps.storage)?),
    }
}

mod query {
    use super::*;

    use crate::msg::{LastExecutedSetResponse, VoteInfo, VoteResponse};
    use dao_interface::voting::InfoResponse;

    pub fn info(deps: Deps) -> StdResult<InfoResponse> {
        let info = cw2::get_contract_version(deps.storage)?;
        Ok(InfoResponse { info })
    }

    fn to_gauge_response(gauge_id: GaugeId, gauge: Gauge) -> GaugeResponse {
        GaugeResponse {
            id: gauge_id,
            title: gauge.title,
            adapter: gauge.adapter.to_string(),
            epoch_size: gauge.epoch,
            min_percent_selected: gauge.min_percent_selected,
            max_options_selected: gauge.max_options_selected,
            max_available_percentage: gauge.max_available_percentage,
            is_stopped: gauge.is_stopped,
            next_epoch: gauge.next_epoch,
            reset: gauge.reset,
            total_epochs: gauge.total_epoch,
        }
    }

    pub fn gauge(deps: Deps, gauge_id: GaugeId) -> StdResult<GaugeResponse> {
        let gauge = GAUGES.load(deps.storage, gauge_id)?;
        Ok(to_gauge_response(gauge_id, gauge))
    }

    // settings for pagination
    pub const MAX_LIMIT: u32 = 100;
    pub const DEFAULT_LIMIT: u32 = 30;

    pub fn list_gauges(
        deps: Deps,
        start_after: Option<u64>,
        limit: Option<u32>,
    ) -> StdResult<ListGaugesResponse> {
        let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
        let start = start_after.map(Bound::exclusive);

        Ok(ListGaugesResponse {
            gauges: GAUGES
                .range(deps.storage, start, None, Order::Ascending)
                .map(|item| {
                    let (id, gauge) = item?;
                    Ok(to_gauge_response(id, gauge))
                })
                .take(limit)
                .collect::<StdResult<Vec<GaugeResponse>>>()?,
        })
    }

    pub fn vote(deps: Deps, gauge_id: u64, voter: String) -> StdResult<VoteResponse> {
        let voter_addr = deps.api.addr_validate(&voter)?;
        let gauge = GAUGES.load(deps.storage, gauge_id)?;

        let vote = votes()
            .may_load(deps.storage, &voter_addr, gauge_id)?
            .filter(|v| !v.is_expired(&gauge))
            .map(|v| VoteInfo {
                voter,
                votes: v.votes,
                cast: v.cast,
            });
        Ok(VoteResponse { vote })
    }

    pub fn list_votes(
        deps: Deps,
        gauge_id: u64,
        start_after: Option<String>,
        limit: Option<u32>,
    ) -> StdResult<ListVotesResponse> {
        Ok(ListVotesResponse {
            votes: votes().query_votes_by_gauge(deps, gauge_id, start_after, limit)?,
        })
    }

    pub fn list_options(
        deps: Deps,
        gauge_id: u64,
        start_after: Option<String>,
        limit: Option<u32>,
    ) -> StdResult<ListOptionsResponse> {
        let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
        let start_after = start_after.as_ref().map(|s| Bound::exclusive(s.as_str()));

        Ok(ListOptionsResponse {
            options: TALLY
                .prefix(gauge_id)
                .range(deps.storage, start_after, None, Order::Ascending)
                .map(|option| {
                    let (option, power) = option?;
                    Ok((option, Uint128::new(power)))
                })
                .take(limit)
                .collect::<StdResult<Vec<(String, Uint128)>>>()?,
        })
    }

    pub fn selected_set(deps: Deps, gauge_id: u64) -> StdResult<SelectedSetResponse> {
        let gauge = GAUGES.load(deps.storage, gauge_id)?;
        let total_cast = TOTAL_CAST.load(deps.storage, gauge_id)?;

        if gauge.is_resetting() || total_cast == 0 {
            return Ok(SelectedSetResponse { votes: vec![] });
        }

        // This is sorted index, but requires manual filtering - cannot be prefixed
        // given our requirements
        let votes = OPTION_BY_POINTS
            .sub_prefix(gauge_id)
            .range(deps.storage, None, None, Order::Descending)
            .filter(|o| {
                let ((power, _), _) = o.as_ref().unwrap();
                if let Some(min_percent_selected) = gauge.min_percent_selected {
                    Decimal::from_ratio(*power, total_cast) >= min_percent_selected
                } else {
                    // filter out options without a vote
                    *power != 0u128
                }
            })
            .map(|o| {
                let ((power, option), _) = o?;
                // If gauge has max_available_percentage set, discard all power
                // above that percentage
                if let Some(max_available_percentage) = gauge.max_available_percentage {
                    if Decimal::from_ratio(power, total_cast) > max_available_percentage {
                        // If power is above available percentage, cut power down to max available
                        return Ok((option, Uint128::new(total_cast) * max_available_percentage));
                    }
                }
                Ok((option, Uint128::new(power)))
            })
            .take(gauge.max_options_selected as usize)
            .collect::<StdResult<Vec<(String, Uint128)>>>()?;

        Ok(SelectedSetResponse { votes })
    }

    pub fn last_executed_set(deps: Deps, gauge_id: u64) -> StdResult<LastExecutedSetResponse> {
        let gauge = GAUGES.load(deps.storage, gauge_id)?;
        Ok(LastExecutedSetResponse {
            votes: gauge.last_executed_set,
        })
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, env: Env, msg: MigrateMsg) -> Result<Response, ContractError> {
    ensure_from_older_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    for (gauge_id, config) in msg.gauge_config.unwrap_or_default() {
        GAUGES.update(deps.storage, gauge_id, |gauge| -> StdResult<_> {
            let mut gauge = gauge.ok_or(StdError::NotFound {
                kind: format!("Gauge with id {}", gauge_id),
            })?;
            if let Some(next_epoch) = config.next_epoch {
                if next_epoch < env.block.time.seconds() {
                    return Err(StdError::GenericErr {
                        msg: "Next epoch value cannot be earlier then current epoch!".to_owned(),
                    });
                }
                gauge.next_epoch = next_epoch;
            }
            if let Some(reset_config) = config.reset {
                if reset_config.next_reset < env.block.time.seconds() {
                    return Err(StdError::GenericErr {
                        msg: "Next reset value cannot be earlier then current epoch!".to_owned(),
                    });
                }
                gauge.reset = Some(Reset {
                    last: gauge.reset.map(|r| r.last).unwrap_or_default(),
                    reset_each: reset_config.reset_epoch,
                    next: reset_config.next_reset,
                });
            }
            Ok(gauge)
        })?;
    }

    Ok(Response::new())
}
