use cosmwasm_std::{to_json_binary, Addr, Uint128, WasmMsg};
use cw20::Cw20Coin;
use cw_multi_test::{next_block, App, Executor};
use cw_utils::Duration;
use dao_interface::query::{GetItemResponse, ProposalModuleCountResponse};
use dao_testing::contracts::{
    cw20_base_contract, cw20_stake_contract, dao_dao_core_contract, dao_proposal_single_contract,
    dao_voting_cw20_staked_contract,
    v1::{cw_core_v1_contract, cw_proposal_single_v1_contract},
};
use dao_voting::veto::VetoConfig;
use dao_voting::{
    deposit::{UncheckedDepositInfo, VotingModuleTokenType},
    status::Status,
};

use crate::testing::queries::query_list_proposals;
use crate::testing::{
    execute::{execute_proposal, make_proposal, vote_on_proposal},
    instantiate::get_pre_propose_info,
    queries::{query_proposal, query_proposal_count},
};

/// This test attempts to simulate a realistic migration from DAO DAO
/// v1 to v2. Other tests in `/tests/tests.rs` check that versions and
/// top-level configs are updated correctly during migration. This
/// concerns itself more with more subtle state in the contracts that
/// is less functionality critical and thus more likely to be
/// overlooked in migration logic.
///
/// - I can migrate with tokens in the treasury and completed
///   proposals.
///
/// - I can migrate an open and unexecutable proposal, and use
///   `close_proposal_on_execution_failure` to close it once the
///   migration completes.
///
/// - Proposal count remains accurate after proposal migration.
///
/// - Items are not overriden during migration.
#[test]
fn test_v1_v2_full_migration() {
    let sender = Addr::unchecked("sender");

    let mut app = App::default();

    // ----
    // instantiate a v1 DAO
    // ----

    let proposal_code = app.store_code(cw_proposal_single_v1_contract());
    let core_code = app.store_code(cw_core_v1_contract());

    // cw20 staking and voting module has not changed across v1->v2 so
    // we use the current edition.
    let cw20_code = app.store_code(cw20_base_contract());
    let cw20_stake_code = app.store_code(cw20_stake_contract());
    let voting_code = app.store_code(dao_voting_cw20_staked_contract());

    let initial_balances = vec![Cw20Coin {
        address: sender.to_string(),
        amount: Uint128::new(2),
    }];

    let core = app
        .instantiate_contract(
            core_code,
            sender.clone(),
            &cw_core_v1::msg::InstantiateMsg {
                admin: Some(sender.to_string()),
                name: "n".to_string(),
                description: "d".to_string(),
                image_url: Some("i".to_string()),
                automatically_add_cw20s: false,
                automatically_add_cw721s: true,
                voting_module_instantiate_info: cw_core_v1::msg::ModuleInstantiateInfo {
                    code_id: voting_code,
                    msg: to_json_binary(&dao_voting_cw20_staked::msg::InstantiateMsg {
                        active_threshold: None,
                        token_info: dao_voting_cw20_staked::msg::TokenInfo::New {
                            code_id: cw20_code,
                            label: "token".to_string(),
                            name: "name".to_string(),
                            symbol: "symbol".to_string(),
                            decimals: 6,
                            initial_balances,
                            marketing: None,
                            staking_code_id: cw20_stake_code,
                            unstaking_duration: None,
                            initial_dao_balance: Some(Uint128::new(100)),
                        },
                    })
                    .unwrap(),
                    admin: cw_core_v1::msg::Admin::CoreContract {},
                    label: "voting".to_string(),
                },
                proposal_modules_instantiate_info: vec![cw_core_v1::msg::ModuleInstantiateInfo {
                    code_id: proposal_code,
                    msg: to_json_binary(&cw_proposal_single_v1::msg::InstantiateMsg {
                        threshold: voting_v1::Threshold::AbsolutePercentage {
                            percentage: voting_v1::PercentageThreshold::Majority {},
                        },
                        max_voting_period: cw_utils_v1::Duration::Height(6),
                        min_voting_period: None,
                        only_members_execute: false,
                        allow_revoting: false,
                        deposit_info: None,
                    })
                    .unwrap(),
                    admin: cw_core_v1::msg::Admin::CoreContract {},
                    label: "proposal".to_string(),
                }],
                initial_items: Some(vec![cw_core_v1::msg::InitialItem {
                    key: "key".to_string(),
                    value: "value".to_string(),
                }]),
            },
            &[],
            "core",
            Some(sender.to_string()),
        )
        .unwrap();
    app.execute(
        sender.clone(),
        WasmMsg::UpdateAdmin {
            contract_addr: core.to_string(),
            admin: core.to_string(),
        }
        .into(),
    )
    .unwrap();

    // ----
    // stake tokens in the DAO
    // ----

    let token = {
        let voting: Addr = app
            .wrap()
            .query_wasm_smart(&core, &cw_core_v1::msg::QueryMsg::VotingModule {})
            .unwrap();
        let staking: Addr = app
            .wrap()
            .query_wasm_smart(
                &voting,
                &dao_voting_cw20_staked::msg::QueryMsg::StakingContract {},
            )
            .unwrap();
        let token: Addr = app
            .wrap()
            .query_wasm_smart(
                &voting,
                &dao_voting_cw20_staked::msg::QueryMsg::TokenContract {},
            )
            .unwrap();
        app.execute_contract(
            sender.clone(),
            token.clone(),
            &cw20::Cw20ExecuteMsg::Send {
                contract: staking.into_string(),
                amount: Uint128::new(1),
                msg: to_json_binary(&cw20_stake::msg::ReceiveMsg::Stake {}).unwrap(),
            },
            &[],
        )
        .unwrap();
        app.update_block(next_block);
        token
    };

    // ----
    // create a proposal and add tokens to the treasury.
    // ----

    let proposal = {
        let modules: Vec<Addr> = app
            .wrap()
            .query_wasm_smart(
                &core,
                &cw_core_v1::msg::QueryMsg::ProposalModules {
                    start_at: None,
                    limit: None,
                },
            )
            .unwrap();
        assert!(modules.len() == 1);
        modules.into_iter().next().unwrap()
    };

    app.execute_contract(
        sender.clone(),
        proposal.clone(),
        &cw_proposal_single_v1::msg::ExecuteMsg::Propose {
            title: "t".to_string(),
            description: "d".to_string(),
            msgs: vec![WasmMsg::Execute {
                contract_addr: core.to_string(),
                msg: to_json_binary(&cw_core_v1::msg::ExecuteMsg::UpdateCw20List {
                    to_add: vec![token.to_string()],
                    to_remove: vec![],
                })
                .unwrap(),
                funds: vec![],
            }
            .into()],
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        sender.clone(),
        proposal.clone(),
        &cw_proposal_single_v1::msg::ExecuteMsg::Vote {
            proposal_id: 1,
            vote: voting_v1::Vote::Yes,
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        sender.clone(),
        proposal.clone(),
        &cw_proposal_single_v1::msg::ExecuteMsg::Execute { proposal_id: 1 },
        &[],
    )
    .unwrap();
    let tokens: Vec<cw_core_v1::query::Cw20BalanceResponse> = app
        .wrap()
        .query_wasm_smart(
            &core,
            &cw_core_v1::msg::QueryMsg::Cw20Balances {
                start_at: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(
        tokens,
        vec![cw_core_v1::query::Cw20BalanceResponse {
            addr: token.clone(),
            balance: Uint128::new(100),
        }]
    );

    // ----
    // Create a proposal that is unexecutable without close_proposal_on_execution_failure
    // ----

    app.execute_contract(
        sender.clone(),
        proposal.clone(),
        &cw_proposal_single_v1::msg::ExecuteMsg::Propose {
            title: "t".to_string(),
            description: "d".to_string(),
            msgs: vec![WasmMsg::Execute {
                contract_addr: token.to_string(),
                msg: to_json_binary(&cw20::Cw20ExecuteMsg::Transfer {
                    recipient: sender.to_string(),
                    // more tokens than the DAO posseses.
                    amount: Uint128::new(101),
                })
                .unwrap(),
                funds: vec![],
            }
            .into()],
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        sender.clone(),
        proposal.clone(),
        &cw_proposal_single_v1::msg::ExecuteMsg::Vote {
            proposal_id: 2,
            vote: voting_v1::Vote::Yes,
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        sender.clone(),
        proposal.clone(),
        &cw_proposal_single_v1::msg::ExecuteMsg::Execute { proposal_id: 2 },
        &[],
    )
    // can not be executed.
    .unwrap_err();
    let cw_proposal_single_v1::query::ProposalResponse {
        proposal: cw_proposal_single_v1::proposal::Proposal { status, .. },
        ..
    } = app
        .wrap()
        .query_wasm_smart(
            &proposal,
            &cw_proposal_single_v1::msg::QueryMsg::Proposal { proposal_id: 2 },
        )
        .unwrap();
    assert_eq!(status, voting_v1::Status::Passed);

    // ----
    // create a proposal to migrate to v2
    // ----

    let v2_core_code = app.store_code(dao_dao_core_contract());
    let v2_proposal_code = app.store_code(dao_proposal_single_contract());

    let pre_propose_info = get_pre_propose_info(
        &mut app,
        Some(UncheckedDepositInfo {
            denom: dao_voting::deposit::DepositToken::VotingModuleToken {
                token_type: VotingModuleTokenType::Cw20,
            },
            amount: Uint128::new(1),
            refund_policy: dao_voting::deposit::DepositRefundPolicy::OnlyPassed,
        }),
        false,
    );

    // now migrate with valid config
    app.execute_contract(
        sender.clone(),
        proposal.clone(),
        &cw_proposal_single_v1::msg::ExecuteMsg::Propose {
            title: "t".to_string(),
            description: "d".to_string(),
            msgs: vec![
                WasmMsg::Migrate {
                    contract_addr: core.to_string(),
                    new_code_id: v2_core_code,
                    msg: to_json_binary(&dao_interface::msg::MigrateMsg::FromV1 {
                        dao_uri: Some("dao-uri".to_string()),
                        params: None,
                    })
                    .unwrap(),
                }
                .into(),
                WasmMsg::Migrate {
                    contract_addr: proposal.to_string(),
                    new_code_id: v2_proposal_code,
                    msg: to_json_binary(&crate::msg::MigrateMsg::FromV1 {
                        close_proposal_on_execution_failure: true,
                        pre_propose_info,
                        veto: Some(VetoConfig {
                            timelock_duration: Duration::Height(10),
                            vetoer: sender.to_string(),
                            early_execute: true,
                            veto_before_passed: false,
                        }),
                    })
                    .unwrap(),
                }
                .into(),
            ],
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        sender.clone(),
        proposal.clone(),
        &cw_proposal_single_v1::msg::ExecuteMsg::Vote {
            proposal_id: 3,
            vote: voting_v1::Vote::Yes,
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        sender.clone(),
        proposal.clone(),
        &cw_proposal_single_v1::msg::ExecuteMsg::Execute { proposal_id: 3 },
        &[],
    )
    .unwrap();

    // ----
    // execute proposal two. the addition of
    // close_proposal_on_execution_failure ought to allow it to close.
    // ----
    execute_proposal(&mut app, &proposal, sender.as_str(), 2);
    let status = query_proposal(&app, &proposal, 2).proposal.status;
    assert_eq!(status, Status::ExecutionFailed);

    // ----
    // check that proposal count is still three after proposal state migration.
    // ----
    let count = query_proposal_count(&app, &proposal);
    assert_eq!(count, 3);

    let migrated_existing_props = query_list_proposals(&app, &proposal, None, None);
    // assert that even though we migrate with a veto config,
    // existing proposals are not affected
    for prop in migrated_existing_props.proposals {
        assert_eq!(prop.proposal.veto, None);
    }
    // ----
    // check that proposal module counts have been updated.
    // ----
    let module_counts: ProposalModuleCountResponse = app
        .wrap()
        .query_wasm_smart(&core, &dao_interface::msg::QueryMsg::ProposalModuleCount {})
        .unwrap();
    assert_eq!(
        module_counts,
        ProposalModuleCountResponse {
            active_proposal_module_count: 1,
            total_proposal_module_count: 1,
        }
    );

    // ----
    // check that items are not overriden in migration.
    // ----
    let item: GetItemResponse = app
        .wrap()
        .query_wasm_smart(
            &core,
            &dao_interface::msg::QueryMsg::GetItem {
                key: "key".to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        item,
        GetItemResponse {
            item: Some("value".to_string())
        }
    );

    // ----
    // check that proposal can still be created an executed.
    // ----
    make_proposal(
        &mut app,
        &proposal,
        sender.as_str(),
        vec![WasmMsg::Execute {
            contract_addr: core.to_string(),
            msg: to_json_binary(&dao_interface::msg::ExecuteMsg::UpdateCw20List {
                to_add: vec![],
                to_remove: vec![token.into_string()],
            })
            .unwrap(),
            funds: vec![],
        }
        .into()],
        None,
    );
    vote_on_proposal(
        &mut app,
        &proposal,
        sender.as_str(),
        4,
        dao_voting::voting::Vote::Yes,
    );

    let new_prop = query_proposal(&app, &proposal, 4);
    assert_eq!(
        new_prop.proposal.veto,
        Some(VetoConfig {
            timelock_duration: Duration::Height(10),
            vetoer: sender.to_string(),
            early_execute: true,
            veto_before_passed: false,
        })
    );

    execute_proposal(&mut app, &proposal, sender.as_str(), 4);
    let tokens: Vec<dao_interface::query::Cw20BalanceResponse> = app
        .wrap()
        .query_wasm_smart(
            &core,
            &dao_interface::msg::QueryMsg::Cw20Balances {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert!(tokens.is_empty())
}
