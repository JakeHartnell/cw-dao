use std::borrow::BorrowMut;

use cosmwasm_std::Addr;
use cw_multi_test::Executor;
use dao_interface::{query::SubDao, state::ProposalModuleStatus};

use crate::{
    testing::{
        helpers::ExecuteParams,
        helpers::VotingType,
        setup::{execute_migration, execute_migration_from_core, setup_dao_v1},
        state_helpers::{
            query_state_v1_cw20, query_state_v1_cw4, query_state_v2_cw20, query_state_v2_cw4,
        },
    },
    types::ProposalParams,
};
use dao_migrator::ContractError;

use super::{helpers::demo_contract, setup::setup_dao_v1_multiple_proposals};

pub fn basic_test(voting_type: VotingType, from_core: bool) {
    let (mut app, module_addrs, v1_code_ids) = setup_dao_v1(voting_type.clone());

    let mut test_state_v1 = match voting_type {
        VotingType::Cw4 => query_state_v1_cw4(
            &mut app,
            module_addrs.proposals[0].clone(),
            module_addrs.voting.clone(),
        ),
        VotingType::Cw20 => query_state_v1_cw20(
            &mut app,
            module_addrs.proposals[0].clone(),
            module_addrs.voting.clone(),
        ),
        VotingType::Cw20V03 => query_state_v1_cw20(
            &mut app,
            module_addrs.proposals[0].clone(),
            module_addrs.voting.clone(),
        ),
    };
    //NOTE: We add 1 to count because we create a new proposal in execute_migration
    test_state_v1.proposal_count += 1;

    match from_core {
        true => {
            execute_migration_from_core(app.borrow_mut(), &module_addrs, v1_code_ids, None).unwrap()
        }
        false => {
            execute_migration(app.borrow_mut(), &module_addrs, v1_code_ids, None, None).unwrap()
        }
    };

    let test_state_v2 = match voting_type {
        VotingType::Cw4 => query_state_v2_cw4(
            &mut app,
            module_addrs.proposals[0].clone(),
            module_addrs.voting,
        ),
        VotingType::Cw20 => query_state_v2_cw20(
            &mut app,
            module_addrs.proposals[0].clone(),
            module_addrs.voting,
        ),
        VotingType::Cw20V03 => query_state_v2_cw20(
            &mut app,
            module_addrs.proposals[0].clone(),
            module_addrs.voting,
        ),
    };

    assert_eq!(test_state_v1, test_state_v2);

    let modules: Vec<dao_interface::state::ProposalModule> = app
        .wrap()
        .query_wasm_smart(
            module_addrs.core,
            &dao_interface::msg::QueryMsg::ProposalModules {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(modules.len(), 2);
    assert_eq!(modules[0].address, module_addrs.proposals[0]);
    assert_eq!(modules[1].status, ProposalModuleStatus::Disabled);
}

#[test]
fn test_execute_migration() {
    // Test basic migrator (not called from core)
    basic_test(VotingType::Cw20, false);
    basic_test(VotingType::Cw4, false);
    basic_test(VotingType::Cw20V03, false);

    // Test basic migrator (called from core)
    basic_test(VotingType::Cw20, true);
    basic_test(VotingType::Cw4, true);
    basic_test(VotingType::Cw20V03, true);
}

#[test]
fn test_migrator_address_is_first() {
    let (mut app, module_addrs, v1_code_ids) = setup_dao_v1(VotingType::Cw20);

    // We init some demo contracts so we can bump the contract addr to "contract1X"
    // That way, when we do a migration, the newely created migrator contract address
    // will be "contract11", because the proposal module address is "contract4"
    // when we query the dao for "ProposalModules", the migrator address
    // will appear first in the list ("contract11" < "contract4")
    let demo_code_id = app.store_code(demo_contract());
    for _ in 0..6 {
        app.instantiate_contract(
            demo_code_id,
            Addr::unchecked("some"),
            &(),
            &[],
            "demo",
            None,
        )
        .unwrap();
    }

    let mut test_state_v1 = query_state_v1_cw20(
        &mut app,
        module_addrs.proposals[0].clone(),
        module_addrs.voting.clone(),
    );
    //NOTE: We add 1 to count because we create a new proposal in execute_migration
    test_state_v1.proposal_count += 1;

    execute_migration(app.borrow_mut(), &module_addrs, v1_code_ids, None, None).unwrap();

    let test_state_v2 = query_state_v2_cw20(
        &mut app,
        module_addrs.proposals[0].clone(),
        module_addrs.voting,
    );

    assert_eq!(test_state_v1, test_state_v2);

    let modules: Vec<dao_interface::state::ProposalModule> = app
        .wrap()
        .query_wasm_smart(
            module_addrs.core,
            &dao_interface::msg::QueryMsg::ProposalModules {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(modules.len(), 2);
    assert_eq!(modules[1].address, module_addrs.proposals[0]); // proposal module
    assert_eq!(modules[0].status, ProposalModuleStatus::Disabled); // migrator module
}

#[test]
fn test_multiple_proposal_modules() {
    let (mut app, module_addrs, v1_code_ids) = setup_dao_v1_multiple_proposals();

    let mut test_state_v1 = query_state_v1_cw20(
        &mut app,
        module_addrs.proposals[0].clone(),
        module_addrs.voting.clone(),
    );
    //NOTE: We add 1 to count because we create a new proposal in execute_migration
    test_state_v1.proposal_count += 1;

    execute_migration(app.borrow_mut(), &module_addrs, v1_code_ids, None, None).unwrap();

    let test_state_v2 = query_state_v2_cw20(
        &mut app,
        module_addrs.proposals[0].clone(),
        module_addrs.voting,
    );

    assert_eq!(test_state_v1, test_state_v2);
}

#[test]
fn test_duplicate_proposal_params() {
    let (mut app, module_addrs, v1_code_ids) = setup_dao_v1_multiple_proposals();

    // 2 pararms with the same addr
    let custom_params = vec![
        (
            module_addrs.proposals[0].to_string(),
            ProposalParams {
                close_proposal_on_execution_failure: true,
                pre_propose_info: dao_voting::pre_propose::PreProposeInfo::AnyoneMayPropose {},
                veto: None,
            },
        ),
        (
            module_addrs.proposals[0].to_string(),
            ProposalParams {
                close_proposal_on_execution_failure: true,
                pre_propose_info: dao_voting::pre_propose::PreProposeInfo::AnyoneMayPropose {},
                veto: None,
            },
        ),
    ];

    let err = execute_migration(
        app.borrow_mut(),
        &module_addrs,
        v1_code_ids,
        None,
        Some(custom_params),
    )
    .unwrap_err()
    .downcast::<ContractError>()
    .unwrap();

    assert_eq!(err, ContractError::DuplicateProposalParams)
}

#[test]
fn test_multiple_proposal_modules_failing() {
    // Test single proposal with multiple proposal params.
    let (mut app, mut module_addrs, v1_code_ids) = setup_dao_v1(VotingType::Cw20);

    // `module_addrs.proposals` is only used to set migration params based on how many proposals we have here
    // Its safe to add/remove 2nd proposal in this tests because the actual proposals are taken within the contract
    // and are not provided externally
    module_addrs.proposals.push(Addr::unchecked("proposal2"));
    let err = execute_migration(app.borrow_mut(), &module_addrs, v1_code_ids, None, None)
        .unwrap_err()
        .downcast::<ContractError>()
        .unwrap();
    assert_eq!(
        err,
        ContractError::MigrationParamsNotEqualProposalModulesLength
    );

    // Test multiple proposals with single proposal params.
    let (mut app, mut module_addrs, v1_code_ids) = setup_dao_v1_multiple_proposals();

    module_addrs.proposals.remove(1);
    let err = execute_migration(app.borrow_mut(), &module_addrs, v1_code_ids, None, None)
        .unwrap_err()
        .downcast::<ContractError>()
        .unwrap();
    assert_eq!(
        err,
        ContractError::MigrationParamsNotEqualProposalModulesLength
    );
}

#[test]
fn test_wrong_code_id() {
    let (mut app, module_addrs, mut v1_code_ids) = setup_dao_v1(VotingType::Cw20);
    let old_v1_code_ids = v1_code_ids.clone();
    v1_code_ids.proposal_single = 555;
    let err = execute_migration(app.borrow_mut(), &module_addrs, v1_code_ids, None, None)
        .unwrap_err()
        .downcast::<ContractError>()
        .unwrap();
    assert_eq!(
        err,
        ContractError::CantMigrateModule {
            code_id: old_v1_code_ids.proposal_single
        }
    );

    let (mut app, module_addrs, mut v1_code_ids) = setup_dao_v1(VotingType::Cw20);
    let old_v1_code_ids = v1_code_ids.clone();
    v1_code_ids.cw20_stake = 555;
    let err = execute_migration(app.borrow_mut(), &module_addrs, v1_code_ids, None, None)
        .unwrap_err()
        .downcast::<ContractError>()
        .unwrap();
    assert_eq!(
        err,
        ContractError::CantMigrateModule {
            code_id: old_v1_code_ids.cw20_stake
        }
    );

    let (mut app, module_addrs, mut v1_code_ids) = setup_dao_v1(VotingType::Cw20);
    v1_code_ids.cw20_staked_balances_voting = 555;
    let err = execute_migration(app.borrow_mut(), &module_addrs, v1_code_ids, None, None)
        .unwrap_err()
        .downcast::<ContractError>()
        .unwrap();
    assert_eq!(err, ContractError::VotingModuleNotFound);

    let (mut app, module_addrs, mut v1_code_ids) = setup_dao_v1(VotingType::Cw4);
    v1_code_ids.cw4_voting = 555;
    let err = execute_migration(app.borrow_mut(), &module_addrs, v1_code_ids, None, None)
        .unwrap_err()
        .downcast::<ContractError>()
        .unwrap();
    assert_eq!(err, ContractError::VotingModuleNotFound);
}

#[test]
fn test_dont_migrate_cw20() {
    let (mut app, module_addrs, v1_code_ids) = setup_dao_v1(VotingType::Cw20);

    let err = execute_migration(
        app.borrow_mut(),
        &module_addrs,
        v1_code_ids.clone(),
        Some(ExecuteParams {
            sub_daos: Some(vec![]),
            migrate_cw20: None,
        }),
        None,
    )
    .unwrap_err()
    .downcast::<ContractError>()
    .unwrap();
    assert_eq!(err, ContractError::DontMigrateCw20);

    let err = execute_migration(
        app.borrow_mut(),
        &module_addrs,
        v1_code_ids,
        Some(ExecuteParams {
            sub_daos: Some(vec![]),
            migrate_cw20: Some(false),
        }),
        None,
    )
    .unwrap_err()
    .downcast::<ContractError>()
    .unwrap();
    assert_eq!(err, ContractError::DontMigrateCw20);
}

#[test]
fn test_sub_daos() {
    let (mut app, module_addrs, v1_code_ids) = setup_dao_v1(VotingType::Cw20);
    let sub_dao = SubDao {
        addr: "sub_dao_1".to_string(),
        charter: None,
    };

    execute_migration(
        app.borrow_mut(),
        &module_addrs,
        v1_code_ids,
        Some(ExecuteParams {
            sub_daos: Some(vec![sub_dao.clone()]),
            migrate_cw20: Some(true),
        }),
        None,
    )
    .unwrap();

    let sub_daos: Vec<dao_interface::query::SubDao> = app
        .wrap()
        .query_wasm_smart(
            module_addrs.core,
            &dao_interface::msg::QueryMsg::ListSubDaos {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(sub_daos, vec![sub_dao]);
}
