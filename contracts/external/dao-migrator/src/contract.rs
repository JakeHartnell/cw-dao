use std::env;

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Addr, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Reply, Response,
    StdResult, SubMsg, WasmMsg,
};
use cw2::set_contract_version;
use dao_core::state::ProposalModule;
use dao_interface::ModuleInstantiateCallback;

use crate::{
    error::ContractError,
    msg::{ExecuteMsg, InstantiateMsg, QueryMsg},
    state::{MODULES_ADDRS, TEST_STATE},
    types::{
        CodeIdPair, MigrationMsgs, MigrationParams, ModulesAddrs, TestState, V1CodeIds, V2CodeIds,
    },
    utils::state_queries::{
        query_proposal_count_v1, query_proposal_count_v2, query_proposal_v1, query_proposal_v2,
        query_single_voting_power_v1, query_single_voting_power_v2, query_total_voting_power_v1,
        query_total_voting_power_v2,
    },
};

pub(crate) const CONTRACT_NAME: &str = "crates.io:dao-migrator";
pub(crate) const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub(crate) const V1_V2_REPLY_ID: u64 = 1;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    let mut msgs: Vec<CosmosMsg> = vec![WasmMsg::Execute {
        contract_addr: info.sender.to_string(),
        msg: to_binary(&ExecuteMsg::MigrateV1ToV2 {
            params: msg.migration_params,
            v1_code_ids: msg.v1_code_ids,
            v2_code_ids: msg.v2_code_ids,
        })?,
        funds: vec![],
    }
    .into()];

    // Add sub daos to core
    if let Some(to_add) = msg.sub_daos {
        msgs.push(
            WasmMsg::Execute {
                contract_addr: env.contract.address.to_string(),
                msg: to_binary(&dao_core::msg::ExecuteMsg::UpdateSubDaos {
                    to_add,
                    to_remove: vec![],
                })?,
                funds: vec![],
            }
            .into(),
        )
    };

    // FINALLY remove the migrator from the core
    msgs.push(
        WasmMsg::Execute {
            contract_addr: info.sender.to_string(),
            msg: to_binary(&dao_core::msg::ExecuteMsg::UpdateProposalModules {
                to_add: vec![],
                to_disable: vec![env.contract.address.to_string()],
            })?,
            funds: vec![],
        }
        .into(),
    );

    Ok(Response::default().set_data(to_binary(&ModuleInstantiateCallback { msgs })?))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::MigrateV1ToV2 {
            params,
            v1_code_ids,
            v2_code_ids,
        } => execute_migration_v1_v2(deps, env, info, params, v1_code_ids, v2_code_ids),
        ExecuteMsg::Conjunction { operands } => Ok(Response::default().add_messages(operands)),
    }
}

fn execute_migration_v1_v2(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    migration_params: MigrationParams,
    v1_code_ids: V1CodeIds,
    v2_code_ids: V2CodeIds,
) -> Result<Response, ContractError> {
    // List of code ids pairs we got and the migration msg of each one of them.
    let proposal_pair = CodeIdPair::new(
        v1_code_ids.proposal_single,
        v2_code_ids.proposal_single,
        MigrationMsgs::DaoProposalSingle(dao_proposal_single::msg::MigrateMsg::FromV1 {
            close_proposal_on_execution_failure: migration_params
                .close_proposal_on_execution_failure,
            pre_propose_info: migration_params.pre_propose_info,
        }),
    ); // cw-proposal-single -> dao_proposal_single
    let voting_pairs: Vec<CodeIdPair> = vec![
        CodeIdPair::new(
            v1_code_ids.cw4_voting,
            v2_code_ids.cw4_voting,
            MigrationMsgs::DaoVotingCw4(dao_voting_cw4::msg::MigrateMsg {}),
        ), // cw4-voting -> dao_voting_cw4
        CodeIdPair::new(
            v1_code_ids.cw20_staked_balances_voting,
            v2_code_ids.cw20_staked_balances_voting,
            MigrationMsgs::DaoVotingCw20Staked(dao_voting_cw20_staked::msg::MigrateMsg {}),
        ), // cw20-staked-balances-voting -> dao-voting-cw20-staked
    ];
    let staking_pair = CodeIdPair::new(
        v1_code_ids.cw20_stake,
        v2_code_ids.cw20_stake,
        MigrationMsgs::Cw20Stake(cw20_stake::msg::MigrateMsg::FromV1 {}),
    ); // cw20-stake -> cw20_stake

    let mut msgs: Vec<WasmMsg> = vec![];
    let mut proposal_error: Option<ContractError> = None;
    let mut modules_addrs = ModulesAddrs::default();

    // --------------------
    // verify voting module
    // --------------------
    let voting_module: Addr = deps.querier.query_wasm_smart(
        info.sender.clone(),
        &dao_core::msg::QueryMsg::VotingModule {},
    )?;

    let voting_code_id =
        if let Ok(contract_info) = deps.querier.query_wasm_contract_info(voting_module.clone()) {
            contract_info.code_id
        } else {
            // Return false if we don't get contract info, means something went wrong.
            return Err(ContractError::NoContractInfo {
                address: voting_module.into(),
            });
        };

    if let Some(voting_pair) = voting_pairs
        .into_iter()
        .find(|x| x.v1_code_id == voting_code_id)
    {
        msgs.push(WasmMsg::Migrate {
            contract_addr: voting_module.to_string(),
            new_code_id: voting_pair.v2_code_id,
            msg: to_binary(&voting_pair.migrate_msg).unwrap(),
        });
        modules_addrs.voting = Some(voting_module.to_string());

        // If voting module is staked cw20, we check that they confirmed migration
        // and migrate the cw20_staked module
        if let MigrationMsgs::DaoVotingCw20Staked(_) = voting_pair.migrate_msg {
            if !migration_params.migrate_stake_cw20_manager.unwrap() {
                return Err(ContractError::DontMigrateCw20);
            }

            let cw20_staked_addr: Addr = deps.querier.query_wasm_smart(
                voting_module,
                &cw20_staked_balance_voting_v1::msg::QueryMsg::StakingContract {},
            )?;

            let c20_staked_code_id = if let Ok(contract_info) = deps
                .querier
                .query_wasm_contract_info(cw20_staked_addr.clone())
            {
                contract_info.code_id
            } else {
                // Return false if we don't get contract info, means something went wrong.
                return Err(ContractError::NoContractInfo {
                    address: cw20_staked_addr.into(),
                });
            };

            // If module is not DAO DAO module
            if c20_staked_code_id != staking_pair.v1_code_id {
                return Err(ContractError::CantMigrateModule {
                    code_id: c20_staked_code_id,
                });
            }

            msgs.push(WasmMsg::Migrate {
                contract_addr: cw20_staked_addr.to_string(),
                new_code_id: staking_pair.v2_code_id,
                msg: to_binary(&staking_pair.migrate_msg).unwrap(),
            });
        }
    } else {
        return Err(ContractError::VotingModuleNotFound);
    }

    // -----------------------
    // verify proposal modules
    // -----------------------
    // We take all the proposal modules of the DAO.
    let proposal_modules: Vec<ProposalModule> = deps.querier.query_wasm_smart(
        info.sender,
        &dao_core::msg::QueryMsg::ProposalModules {
            start_after: None,
            limit: None,
        },
    )?;

    let success_proposal = proposal_modules.into_iter().all(|module| {
        // Make sure that we don't try to migrate the migrator ...
        if env.contract.address == module.address {
            return true;
        }
        // Get the code id of the module
        let proposal_code_id = if let Ok(contract_info) = deps
            .querier
            .query_wasm_contract_info(module.address.clone())
        {
            contract_info.code_id
        } else {
            // Return false if we don't get contract info, means something went wrong.
            proposal_error = Some(ContractError::NoContractInfo {
                address: module.address.into(),
            });
            return false;
        };

        // check if Code id is valid DAO DAO code id
        if proposal_code_id == proposal_pair.v1_code_id {
            msgs.push(WasmMsg::Migrate {
                contract_addr: module.address.to_string(),
                new_code_id: proposal_pair.v2_code_id,
                msg: to_binary(&proposal_pair.migrate_msg).unwrap(),
            });
            modules_addrs.proposals.push(module.address.to_string());
        } else {
            // Return false because we couldn't find the code id on our list.
            proposal_error = Some(ContractError::CantMigrateModule {
                code_id: proposal_code_id,
            });
            return false;
        }

        true
    });

    if !success_proposal {
        Err(proposal_error.unwrap())
    } else {
        // We successfully verified all modules of the DAO, we can send migration msgs.

        // Verify we got voting address, and at least 1 proposal single address
        modules_addrs.verify(deps.as_ref())?;
        MODULES_ADDRS.save(deps.storage, &modules_addrs)?;
        // Do the state query, and save it in storage
        let state = query_state_v1(deps.as_ref(), modules_addrs)?;
        TEST_STATE.save(deps.storage, &state)?;

        // Create the conjuction msg.
        let conjuction_msg = SubMsg::reply_on_success(
            WasmMsg::Execute {
                contract_addr: env.contract.address.to_string(),
                msg: to_binary(&ExecuteMsg::Conjunction { operands: msgs })?,
                funds: vec![],
            },
            V1_V2_REPLY_ID,
        );

        Ok(Response::default().add_submessage(conjuction_msg))
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(_deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {}
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, _env: Env, reply: Reply) -> Result<Response, ContractError> {
    match reply.id {
        V1_V2_REPLY_ID => {
            // This is called after we got all the migrations successfully
            test_state(deps.as_ref())?;

            Ok(Response::default()
                .add_attribute("action", "migrate")
                .add_attribute("status", "success"))
        }
        _ => Err(ContractError::UnrecognisedReplyId),
    }
}

// TODO: Refactor to match queries based on passed version? or leave it like that?
// We can pass the version we want to query to a single function and let the function handle the right call to make.
fn query_state_v1(deps: Deps, module_addrs: ModulesAddrs) -> Result<TestState, ContractError> {
    // Queries needs to do
    // 1. `query_dump_state` - query dao-core (`DumpState`)to get the `proposal_modules`, `voting_module`, and `total_proposal_module_count`
    // 2. `query_items` - query dao-core `ListItems`.
    // 3. `query_proposal_count` - query all proposal modules with `ProposalCount`
    // 4. `query_proposal` - query all proposal modules with `ReverseProposals`, get 1st proposal, convert it from v1 to v2.
    // 5. `query_total_power` - query voting module for `TotalPowerAtHeight`
    // 6. `query_voting_power` - query proposer at start height with `VotingPowerAtHeight`

    // let core_dump_state = query_core_dump_state_v1(deps, module_addrs.core.as_ref().unwrap())?;
    // let core_items = query_core_items_v1(deps, module_addrs.core.as_ref().unwrap())?;
    let proposal_counts = query_proposal_count_v1(deps, module_addrs.proposals.clone())?;
    let (proposals, last_proposal_data) = query_proposal_v1(deps, module_addrs.proposals)?;
    let total_voting_power = query_total_voting_power_v1(
        deps,
        module_addrs.voting.clone().unwrap(),
        last_proposal_data.start_height,
    )?;
    let single_voting_power = query_single_voting_power_v1(
        deps,
        module_addrs.voting.unwrap(),
        last_proposal_data.proposer,
        last_proposal_data.start_height,
    )?;

    Ok(TestState {
        // core_dump_state,
        // core_items,
        proposal_counts,
        proposals,
        total_voting_power,
        single_voting_power,
    })
}

fn query_state_v2(deps: Deps, module_addrs: ModulesAddrs) -> Result<TestState, ContractError> {
    // let core_dump_state = query_core_dump_state_v2(deps, module_addrs.core.as_ref().unwrap())?;
    // let core_items = query_core_items_v2(deps, module_addrs.core.as_ref().unwrap())?;
    let proposal_counts = query_proposal_count_v2(deps, module_addrs.proposals.clone())?;
    let (proposals, last_proposal_data) = query_proposal_v2(deps, module_addrs.proposals.clone())?;
    let total_voting_power = query_total_voting_power_v2(
        deps,
        module_addrs.voting.clone().unwrap(),
        last_proposal_data.start_height,
    )?;
    let single_voting_power = query_single_voting_power_v2(
        deps,
        module_addrs.voting.unwrap(),
        last_proposal_data.proposer,
        last_proposal_data.start_height,
    )?;

    Ok(TestState {
        // core_dump_state,
        // core_items,
        proposal_counts,
        proposals,
        total_voting_power,
        single_voting_power,
    })
}

fn test_state(deps: Deps) -> Result<(), ContractError> {
    let old_state = TEST_STATE.load(deps.storage)?;
    let modules_addrs = MODULES_ADDRS.load(deps.storage)?;
    let new_state = query_state_v2(deps, modules_addrs)?;

    if new_state == old_state {
        Ok(())
    } else {
        Err(ContractError::TestFailed)
    }
}
