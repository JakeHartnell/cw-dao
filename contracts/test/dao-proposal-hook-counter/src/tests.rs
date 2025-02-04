use cosmwasm_std::{to_json_binary, Addr, Uint128};
use cw20::Cw20Coin;
use cw_hooks::HooksResponse;
use cw_multi_test::{App, Executor};
use dao_interface::state::ProposalModule;
use dao_interface::state::{Admin, ModuleInstantiateInfo};
use dao_testing::contracts::{
    cw20_base_contract, dao_dao_core_contract, dao_proposal_hook_counter_contract,
    dao_proposal_single_contract, dao_voting_cw20_balance_contract,
};
use dao_voting::{
    pre_propose::PreProposeInfo,
    threshold::{PercentageThreshold, Threshold},
    voting::Vote,
};

use crate::msg::{CountResponse, InstantiateMsg, QueryMsg};
use dao_proposal_single::state::Config;
use dao_voting::proposal::SingleChoiceProposeMsg as ProposeMsg;

const CREATOR_ADDR: &str = "creator";

fn instantiate_governance(
    app: &mut App,
    code_id: u64,
    msg: dao_interface::msg::InstantiateMsg,
) -> Addr {
    app.instantiate_contract(
        code_id,
        Addr::unchecked(CREATOR_ADDR),
        &msg,
        &[],
        "cw-governance",
        None,
    )
    .unwrap()
}

fn instantiate_with_default_governance(
    app: &mut App,
    code_id: u64,
    msg: dao_proposal_single::msg::InstantiateMsg,
    initial_balances: Option<Vec<Cw20Coin>>,
) -> Addr {
    let cw20_id = app.store_code(cw20_base_contract());
    let governance_id = app.store_code(dao_dao_core_contract());
    let votemod_id = app.store_code(dao_voting_cw20_balance_contract());

    let initial_balances = initial_balances.unwrap_or_else(|| {
        vec![Cw20Coin {
            address: CREATOR_ADDR.to_string(),
            amount: Uint128::new(100_000_000),
        }]
    });

    let governance_instantiate = dao_interface::msg::InstantiateMsg {
        dao_uri: None,
        admin: None,
        name: "DAO DAO".to_string(),
        description: "A DAO that builds DAOs".to_string(),
        image_url: None,
        automatically_add_cw20s: true,
        automatically_add_cw721s: true,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: votemod_id,
            msg: to_json_binary(&dao_voting_cw20_balance::msg::InstantiateMsg {
                token_info: dao_voting_cw20_balance::msg::TokenInfo::New {
                    code_id: cw20_id,
                    label: "DAO DAO governance token".to_string(),
                    name: "DAO".to_string(),
                    symbol: "DAO".to_string(),
                    decimals: 6,
                    initial_balances,
                    marketing: None,
                },
            })
            .unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "DAO DAO voting module".to_string(),
        },
        proposal_modules_instantiate_info: vec![ModuleInstantiateInfo {
            code_id,
            msg: to_json_binary(&msg).unwrap(),
            admin: Some(Admin::CoreModule {}),
            funds: vec![],
            label: "DAO DAO governance module".to_string(),
        }],
        initial_items: None,
    };

    instantiate_governance(app, governance_id, governance_instantiate)
}

#[test]
fn test_counters() {
    let mut app = App::default();
    let govmod_id = app.store_code(dao_proposal_single_contract());
    let counters_id = app.store_code(dao_proposal_hook_counter_contract());

    let threshold = Threshold::AbsolutePercentage {
        percentage: PercentageThreshold::Majority {},
    };
    let max_voting_period = cw_utils::Duration::Height(6);
    let instantiate = dao_proposal_single::msg::InstantiateMsg {
        threshold,
        max_voting_period,
        min_voting_period: None,
        only_members_execute: false,
        allow_revoting: false,
        pre_propose_info: PreProposeInfo::AnyoneMayPropose {},
        close_proposal_on_execution_failure: true,
        veto: None,
        delegation_module: None,
    };

    let governance_addr =
        instantiate_with_default_governance(&mut app, govmod_id, instantiate, None);
    let governance_modules: Vec<ProposalModule> = app
        .wrap()
        .query_wasm_smart(
            governance_addr,
            &dao_interface::msg::QueryMsg::ProposalModules {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(governance_modules.len(), 1);
    let govmod_single = governance_modules.into_iter().next().unwrap().address;

    let govmod_config: Config = app
        .wrap()
        .query_wasm_smart(
            govmod_single.clone(),
            &dao_proposal_single::msg::QueryMsg::Config {},
        )
        .unwrap();
    let dao = govmod_config.dao;

    let counters: Addr = app
        .instantiate_contract(
            counters_id,
            Addr::unchecked(CREATOR_ADDR),
            &InstantiateMsg {
                should_error: false,
            },
            &[],
            "counters",
            None,
        )
        .unwrap();
    let failing_counters: Addr = app
        .instantiate_contract(
            counters_id,
            Addr::unchecked(CREATOR_ADDR),
            &InstantiateMsg { should_error: true },
            &[],
            "failing counters",
            None,
        )
        .unwrap();

    // Register both hooks
    app.execute_contract(
        dao.clone(),
        govmod_single.clone(),
        &dao_proposal_single::msg::ExecuteMsg::AddProposalHook {
            address: counters.to_string(),
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        dao.clone(),
        govmod_single.clone(),
        &dao_proposal_single::msg::ExecuteMsg::AddVoteHook {
            address: counters.to_string(),
        },
        &[],
    )
    .unwrap();

    // Query both hooks
    let hooks: HooksResponse = app
        .wrap()
        .query_wasm_smart(
            govmod_single.clone(),
            &dao_proposal_single::msg::QueryMsg::ProposalHooks {},
        )
        .unwrap();
    assert_eq!(hooks.hooks.len(), 1);
    let hooks: HooksResponse = app
        .wrap()
        .query_wasm_smart(
            govmod_single.clone(),
            &dao_proposal_single::msg::QueryMsg::VoteHooks {},
        )
        .unwrap();
    assert_eq!(hooks.hooks.len(), 1);

    // Query proposal counter, expect 0
    let resp: CountResponse = app
        .wrap()
        .query_wasm_smart(counters.clone(), &QueryMsg::ProposalCounter {})
        .unwrap();
    assert_eq!(resp.count, 0);

    // Create a new proposal.
    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        govmod_single.clone(),
        &dao_proposal_single::msg::ExecuteMsg::Propose(ProposeMsg {
            title: "A simple text proposal".to_string(),
            description: "This is a simple text proposal".to_string(),
            msgs: vec![],
            proposer: None,
            vote: None,
        }),
        &[],
    )
    .unwrap();

    // Query proposal counter, expect 1
    let resp: CountResponse = app
        .wrap()
        .query_wasm_smart(counters.clone(), &QueryMsg::ProposalCounter {})
        .unwrap();
    assert_eq!(resp.count, 1);

    // Query vote counter, expect 0
    let resp: CountResponse = app
        .wrap()
        .query_wasm_smart(counters.clone(), &QueryMsg::VoteCounter {})
        .unwrap();
    assert_eq!(resp.count, 0);

    // Query status changed counter, expect 0
    let resp: CountResponse = app
        .wrap()
        .query_wasm_smart(counters.clone(), &QueryMsg::StatusChangedCounter {})
        .unwrap();
    assert_eq!(resp.count, 0);

    // Vote
    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        govmod_single.clone(),
        &dao_proposal_single::msg::ExecuteMsg::Vote {
            proposal_id: 1,
            vote: Vote::Yes,
            rationale: None,
        },
        &[],
    )
    .unwrap();

    // Query vote counter, expect 1
    let resp: CountResponse = app
        .wrap()
        .query_wasm_smart(counters.clone(), &QueryMsg::VoteCounter {})
        .unwrap();
    assert_eq!(resp.count, 1);

    // Query status changed counter, expect 1
    let resp: CountResponse = app
        .wrap()
        .query_wasm_smart(counters.clone(), &QueryMsg::StatusChangedCounter {})
        .unwrap();
    assert_eq!(resp.count, 1);

    // Register the failing hooks
    app.execute_contract(
        dao.clone(),
        govmod_single.clone(),
        &dao_proposal_single::msg::ExecuteMsg::AddProposalHook {
            address: failing_counters.to_string(),
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        dao.clone(),
        govmod_single.clone(),
        &dao_proposal_single::msg::ExecuteMsg::AddVoteHook {
            address: failing_counters.to_string(),
        },
        &[],
    )
    .unwrap();

    // Expect 2 for each hook
    let hooks: HooksResponse = app
        .wrap()
        .query_wasm_smart(
            govmod_single.clone(),
            &dao_proposal_single::msg::QueryMsg::ProposalHooks {},
        )
        .unwrap();
    assert_eq!(hooks.hooks.len(), 2);
    let hooks: HooksResponse = app
        .wrap()
        .query_wasm_smart(
            govmod_single.clone(),
            &dao_proposal_single::msg::QueryMsg::VoteHooks {},
        )
        .unwrap();
    assert_eq!(hooks.hooks.len(), 2);

    // Create a new proposal.
    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        govmod_single.clone(),
        &dao_proposal_single::msg::ExecuteMsg::Propose(ProposeMsg {
            title: "A simple text proposal 2nd".to_string(),
            description: "This is a simple text proposal 2nd".to_string(),
            msgs: vec![],
            proposer: None,
            vote: None,
        }),
        &[],
    )
    .unwrap();

    // The success counters should still work
    // Query proposal counter, expect 2
    let resp: CountResponse = app
        .wrap()
        .query_wasm_smart(counters.clone(), &QueryMsg::ProposalCounter {})
        .unwrap();
    assert_eq!(resp.count, 2);

    // The contract should of removed the failing counters
    let hooks: HooksResponse = app
        .wrap()
        .query_wasm_smart(
            govmod_single.clone(),
            &dao_proposal_single::msg::QueryMsg::ProposalHooks {},
        )
        .unwrap();
    assert_eq!(hooks.hooks.len(), 1);

    // To verify it removed the right one, lets try and remove failing counters
    // will fail as it does not exist.
    let _err = app
        .execute_contract(
            dao.clone(),
            govmod_single.clone(),
            &dao_proposal_single::msg::ExecuteMsg::RemoveProposalHook {
                address: failing_counters.to_string(),
            },
            &[],
        )
        .unwrap_err();

    // It should still have the vote hook as that has not technically failed yet
    let hooks: HooksResponse = app
        .wrap()
        .query_wasm_smart(
            govmod_single.clone(),
            &dao_proposal_single::msg::QueryMsg::VoteHooks {},
        )
        .unwrap();
    assert_eq!(hooks.hooks.len(), 2);

    // Vote on the new proposal to fail the other hook
    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        govmod_single.clone(),
        &dao_proposal_single::msg::ExecuteMsg::Vote {
            rationale: None,
            proposal_id: 2,
            vote: Vote::Yes,
        },
        &[],
    )
    .unwrap();

    // The success counters should still work
    // Query vote counter, expect 2
    let resp: CountResponse = app
        .wrap()
        .query_wasm_smart(counters.clone(), &QueryMsg::VoteCounter {})
        .unwrap();
    assert_eq!(resp.count, 2);
    // Query status changed counter, expect 2
    let resp: CountResponse = app
        .wrap()
        .query_wasm_smart(counters, &QueryMsg::StatusChangedCounter {})
        .unwrap();
    assert_eq!(resp.count, 2);

    // The contract should of removed the failing counters
    let hooks: HooksResponse = app
        .wrap()
        .query_wasm_smart(
            govmod_single.clone(),
            &dao_proposal_single::msg::QueryMsg::VoteHooks {},
        )
        .unwrap();
    assert_eq!(hooks.hooks.len(), 1);

    // To verify it removed the right one, lets try and remove failing counters
    // will fail as it does not exist.
    let _err = app
        .execute_contract(
            dao,
            govmod_single.clone(),
            &dao_proposal_single::msg::ExecuteMsg::RemoveVoteHook {
                address: failing_counters.to_string(),
            },
            &[],
        )
        .unwrap_err();

    // Verify only one hook remains for each
    let hooks: HooksResponse = app
        .wrap()
        .query_wasm_smart(
            govmod_single.clone(),
            &dao_proposal_single::msg::QueryMsg::ProposalHooks {},
        )
        .unwrap();
    assert_eq!(hooks.hooks.len(), 1);
    let hooks: HooksResponse = app
        .wrap()
        .query_wasm_smart(
            govmod_single,
            &dao_proposal_single::msg::QueryMsg::VoteHooks {},
        )
        .unwrap();
    assert_eq!(hooks.hooks.len(), 1);
}
