use crate::msg::InstantiateMsg;
use crate::testing::instantiate::get_pre_propose_info;
use crate::testing::{
    execute::{
        close_proposal, execute_proposal, execute_proposal_should_fail, make_proposal, mint_cw20s,
        vote_on_proposal,
    },
    instantiate::{
        get_default_token_dao_proposal_module_instantiate,
        instantiate_with_staked_balances_governance,
    },
    queries::{query_balance_cw20, query_dao_token, query_proposal, query_single_proposal_module},
};
use cosmwasm_std::{to_json_binary, Addr, CosmosMsg, Decimal, Uint128, WasmMsg};
use cw20::Cw20Coin;
use cw_multi_test::{next_block, App};
use cw_utils::Duration;
use dao_voting::{
    deposit::{DepositRefundPolicy, UncheckedDepositInfo, VotingModuleTokenType},
    status::Status,
    threshold::{PercentageThreshold, Threshold::AbsolutePercentage},
    voting::Vote,
};

use super::CREATOR_ADDR;
use crate::query::ProposalResponse;
use dao_proposal_single::ContractError;

struct CommonTest {
    app: App,
    proposal_module: Addr,
    proposal_id: u64,
}
fn setup_test(messages: Vec<CosmosMsg>) -> CommonTest {
    let mut app = App::default();
    let instantiate = get_default_token_dao_proposal_module_instantiate(&mut app);
    let core_addr = instantiate_with_staked_balances_governance(&mut app, instantiate, None);
    let proposal_module = query_single_proposal_module(&app, &core_addr);
    let gov_token = query_dao_token(&app, &core_addr);

    // Mint some tokens to pay the proposal deposit.
    mint_cw20s(&mut app, &gov_token, &core_addr, CREATOR_ADDR, 10_000_000);
    let proposal_id = make_proposal(&mut app, &proposal_module, CREATOR_ADDR, messages, None);

    CommonTest {
        app,
        proposal_module,
        proposal_id,
    }
}

// A proposal that is still accepting votes (is open) cannot
// be executed. Any attempts to do so should fail and return
// an error.
#[test]
fn test_execute_proposal_open() {
    let CommonTest {
        mut app,
        proposal_module,
        proposal_id,
    } = setup_test(vec![]);

    app.update_block(next_block);

    // assert proposal is open
    let proposal = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(proposal.proposal.status, Status::Open);

    // attempt to execute and assert that it fails
    let err = execute_proposal_should_fail(&mut app, &proposal_module, CREATOR_ADDR, proposal_id);
    assert!(matches!(err, ContractError::NotPassed {}))
}

// A proposal can be executed if and only if it passed.
// Any attempts to execute a proposal that has been rejected
// or closed (after rejection) should fail and return an error.
#[test]
fn test_execute_proposal_rejected_closed() {
    let CommonTest {
        mut app,
        proposal_module,
        proposal_id,
    } = setup_test(vec![]);

    // Assert proposal is open and vote enough to reject it
    let proposal: ProposalResponse = query_proposal(&app, &proposal_module, 1);
    assert_eq!(proposal.proposal.status, Status::Open);
    vote_on_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        proposal_id,
        Vote::No,
    );

    app.update_block(next_block);

    // Assert proposal is rejected
    let proposal: ProposalResponse = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(proposal.proposal.status, Status::Rejected);

    // Attempt to execute
    let err = execute_proposal_should_fail(&mut app, &proposal_module, CREATOR_ADDR, proposal_id);
    assert!(matches!(err, ContractError::NotPassed {}));

    app.update_block(next_block);

    // close the proposal
    close_proposal(&mut app, &proposal_module, CREATOR_ADDR, proposal_id);
    let proposal = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(proposal.proposal.status, Status::Closed);

    // Attempt to execute
    let err = execute_proposal_should_fail(&mut app, &proposal_module, CREATOR_ADDR, proposal_id);
    assert!(matches!(err, ContractError::NotPassed {}))
}

// A proposal can only be executed once. Any subsequent
// attempts to execute it should fail and return an error.
#[test]
fn test_execute_proposal_more_than_once() {
    let CommonTest {
        mut app,
        proposal_module,
        proposal_id,
    } = setup_test(vec![]);

    // Assert proposal is open and vote enough to reject it
    let proposal: ProposalResponse = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(proposal.proposal.status, Status::Open);
    vote_on_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        proposal_id,
        Vote::Yes,
    );

    app.update_block(next_block);

    // assert proposal is passed, execute it
    let proposal = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(proposal.proposal.status, Status::Passed);
    execute_proposal(&mut app, &proposal_module, CREATOR_ADDR, proposal_id);

    app.update_block(next_block);

    // assert proposal executed and attempt to execute it again
    let proposal = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(proposal.proposal.status, Status::Executed);
    let err: ContractError =
        execute_proposal_should_fail(&mut app, &proposal_module, CREATOR_ADDR, proposal_id);
    assert!(matches!(err, ContractError::NotPassed {}));
}

// After proposal is executed, no subsequent votes
// should change the status of the proposal, even if
// the votes should shift to the opposing direction.
#[test]
pub fn test_executed_prop_state_remains_after_vote_swing() {
    let mut app = App::default();

    let instantiate = InstantiateMsg {
        veto: None,
        threshold: AbsolutePercentage {
            percentage: PercentageThreshold::Percent(Decimal::percent(15)),
        },
        max_voting_period: Duration::Time(604800), // One week.
        min_voting_period: None,
        only_members_execute: true,
        allow_revoting: false,
        pre_propose_info: get_pre_propose_info(
            &mut app,
            Some(UncheckedDepositInfo {
                denom: dao_voting::deposit::DepositToken::VotingModuleToken {
                    token_type: VotingModuleTokenType::Cw20,
                },
                amount: Uint128::new(10_000_000),
                refund_policy: DepositRefundPolicy::OnlyPassed,
            }),
            false,
        ),
        close_proposal_on_execution_failure: true,
        delegation_module: None,
    };

    let core_addr = instantiate_with_staked_balances_governance(
        &mut app,
        instantiate,
        Some(vec![
            Cw20Coin {
                address: "threshold".to_string(),
                amount: Uint128::new(20),
            },
            Cw20Coin {
                address: CREATOR_ADDR.to_string(),
                amount: Uint128::new(50),
            },
            Cw20Coin {
                address: "overslept_vote".to_string(),
                amount: Uint128::new(30),
            },
        ]),
    );
    let proposal_module = query_single_proposal_module(&app, &core_addr);
    let gov_token = query_dao_token(&app, &core_addr);

    mint_cw20s(&mut app, &gov_token, &core_addr, CREATOR_ADDR, 10_000_000);
    let proposal_id = make_proposal(&mut app, &proposal_module, CREATOR_ADDR, vec![], None);

    // someone quickly votes, proposal gets executed
    vote_on_proposal(
        &mut app,
        &proposal_module,
        "threshold",
        proposal_id,
        Vote::Yes,
    );
    execute_proposal(&mut app, &proposal_module, CREATOR_ADDR, proposal_id);

    app.update_block(next_block);

    // assert prop is executed prior to its expiry
    let proposal = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(proposal.proposal.status, Status::Executed);
    assert_eq!(proposal.proposal.votes.yes, Uint128::new(20));
    assert!(!proposal.proposal.expiration.is_expired(&app.block_info()));

    // someone wakes up and casts their vote to express their
    // opinion (not affecting the result of proposal)
    vote_on_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        proposal_id,
        Vote::No,
    );
    vote_on_proposal(
        &mut app,
        &proposal_module,
        "overslept_vote",
        proposal_id,
        Vote::No,
    );

    app.update_block(next_block);

    // assert that everyone's votes are reflected in the proposal
    // and proposal remains in executed state
    let proposal = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(proposal.proposal.status, Status::Executed);
    assert_eq!(proposal.proposal.votes.yes, Uint128::new(20));
    assert_eq!(proposal.proposal.votes.no, Uint128::new(80));
}

// After reaching a passing state, no subsequent votes
// should change the status of the proposal, even if
// the votes should shift to the opposing direction.
#[test]
pub fn test_passed_prop_state_remains_after_vote_swing() {
    let mut app = App::default();

    let instantiate = InstantiateMsg {
        veto: None,
        threshold: AbsolutePercentage {
            percentage: PercentageThreshold::Percent(Decimal::percent(15)),
        },
        max_voting_period: Duration::Time(604800), // One week.
        min_voting_period: None,
        only_members_execute: true,
        allow_revoting: false,
        pre_propose_info: get_pre_propose_info(
            &mut app,
            Some(UncheckedDepositInfo {
                denom: dao_voting::deposit::DepositToken::VotingModuleToken {
                    token_type: VotingModuleTokenType::Cw20,
                },
                amount: Uint128::new(10_000_000),
                refund_policy: DepositRefundPolicy::OnlyPassed,
            }),
            false,
        ),
        close_proposal_on_execution_failure: true,
        delegation_module: None,
    };

    let core_addr = instantiate_with_staked_balances_governance(
        &mut app,
        instantiate,
        Some(vec![
            Cw20Coin {
                address: "threshold".to_string(),
                amount: Uint128::new(20),
            },
            Cw20Coin {
                address: CREATOR_ADDR.to_string(),
                amount: Uint128::new(50),
            },
            Cw20Coin {
                address: "overslept_vote".to_string(),
                amount: Uint128::new(30),
            },
        ]),
    );
    let proposal_module = query_single_proposal_module(&app, &core_addr);
    let gov_token = query_dao_token(&app, &core_addr);

    // if the proposal passes, it should mint 100_000_000 tokens to "threshold"
    let msg = cw20::Cw20ExecuteMsg::Mint {
        recipient: "threshold".to_string(),
        amount: Uint128::new(100_000_000),
    };
    let binary_msg = to_json_binary(&msg).unwrap();

    mint_cw20s(&mut app, &gov_token, &core_addr, CREATOR_ADDR, 10_000_000);
    let proposal_id = make_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        vec![WasmMsg::Execute {
            contract_addr: gov_token.to_string(),
            msg: binary_msg,
            funds: vec![],
        }
        .into()],
        None,
    );

    // assert that the initial "threshold" address balance is 0
    let balance = query_balance_cw20(&app, gov_token.to_string(), "threshold");
    assert_eq!(balance, Uint128::zero());

    // vote enough to pass the proposal
    vote_on_proposal(
        &mut app,
        &proposal_module,
        "threshold",
        proposal_id,
        Vote::Yes,
    );

    // assert proposal is passed with 20 votes in favor and none opposed
    let proposal = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(proposal.proposal.status, Status::Passed);
    assert_eq!(proposal.proposal.votes.yes, Uint128::new(20));
    assert_eq!(proposal.proposal.votes.no, Uint128::zero());

    app.update_block(next_block);

    // the other voters wake up, vote against the proposal
    vote_on_proposal(
        &mut app,
        &proposal_module,
        CREATOR_ADDR,
        proposal_id,
        Vote::No,
    );
    vote_on_proposal(
        &mut app,
        &proposal_module,
        "overslept_vote",
        proposal_id,
        Vote::No,
    );

    app.update_block(next_block);

    // assert that the late votes have been counted and proposal
    // is still in passed state before executing it
    let proposal = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(proposal.proposal.status, Status::Passed);
    assert_eq!(proposal.proposal.votes.yes, Uint128::new(20));
    assert_eq!(proposal.proposal.votes.no, Uint128::new(80));

    execute_proposal(&mut app, &proposal_module, CREATOR_ADDR, proposal_id);

    app.update_block(next_block);

    // make sure that the initial "threshold" address balance is
    // 100_000_000 and late votes did not make a difference
    let proposal = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(proposal.proposal.status, Status::Executed);
    assert_eq!(proposal.proposal.votes.yes, Uint128::new(20));
    assert_eq!(proposal.proposal.votes.no, Uint128::new(80));
    let balance = query_balance_cw20(&app, gov_token.to_string(), "threshold");
    assert_eq!(balance, Uint128::new(100_000_000));
}
