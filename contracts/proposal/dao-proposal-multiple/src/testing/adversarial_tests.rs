use crate::msg::{ExecuteMsg, InstantiateMsg};
use crate::testing::execute::{make_proposal, mint_cw20s};
use crate::testing::instantiate::{
    _get_default_token_dao_proposal_module_instantiate,
    instantiate_with_multiple_staked_balances_governance,
};
use crate::testing::queries::{
    query_balance_cw20, query_dao_token, query_multiple_proposal_module, query_proposal,
};
use crate::testing::tests::{get_pre_propose_info, ALTERNATIVE_ADDR, CREATOR_ADDR};
use cosmwasm_std::{to_json_binary, Addr, CosmosMsg, Decimal, Uint128, WasmMsg};
use cw20::Cw20Coin;
use cw_multi_test::{next_block, App, Executor};
use cw_utils::Duration;
use dao_proposal_multiple::ContractError;
use dao_voting::{
    deposit::{DepositRefundPolicy, UncheckedDepositInfo, VotingModuleTokenType},
    multiple_choice::{
        MultipleChoiceOption, MultipleChoiceOptions, MultipleChoiceVote, VotingStrategy,
    },
    status::Status,
    threshold::PercentageThreshold,
};

struct CommonTest {
    app: App,
    proposal_module: Addr,
    proposal_id: u64,
}
fn setup_test(_messages: Vec<CosmosMsg>) -> CommonTest {
    let mut app = App::default();
    let instantiate = _get_default_token_dao_proposal_module_instantiate(&mut app);
    let core_addr =
        instantiate_with_multiple_staked_balances_governance(&mut app, instantiate, None);
    let proposal_module = query_multiple_proposal_module(&app, &core_addr);
    let gov_token = query_dao_token(&app, &core_addr);

    // Mint some tokens to pay the proposal deposit.
    mint_cw20s(&mut app, &gov_token, &core_addr, CREATOR_ADDR, 10_000_000);

    let options = vec![
        MultipleChoiceOption {
            title: "title 1".to_string(),
            description: "multiple choice option 1".to_string(),
            msgs: vec![],
        },
        MultipleChoiceOption {
            title: "title 2".to_string(),
            description: "multiple choice option 2".to_string(),
            msgs: vec![],
        },
    ];

    let mc_options = MultipleChoiceOptions { options };

    let proposal_id = make_proposal(&mut app, &proposal_module, CREATOR_ADDR, mc_options, None);

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
    let prop = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(prop.proposal.status, Status::Open);

    // attempt to execute and assert that it fails
    let err = app
        .execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            proposal_module,
            &ExecuteMsg::Execute { proposal_id },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();

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

    app.update_block(next_block);

    // assert proposal is open
    let prop = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(prop.proposal.status, Status::Open);

    // Vote on both options to reject the proposal
    let vote = MultipleChoiceVote { option_id: 0 };
    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        proposal_module.clone(),
        &ExecuteMsg::Vote {
            proposal_id,
            vote,
            rationale: None,
        },
        &[],
    )
    .unwrap();

    let vote = MultipleChoiceVote { option_id: 1 };
    app.execute_contract(
        Addr::unchecked(ALTERNATIVE_ADDR),
        proposal_module.clone(),
        &ExecuteMsg::Vote {
            proposal_id,
            vote,
            rationale: None,
        },
        &[],
    )
    .unwrap();

    app.update_block(next_block);

    let prop = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(prop.proposal.status, Status::Rejected);

    // attempt to execute and assert that it fails
    let err = app
        .execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            proposal_module.clone(),
            &ExecuteMsg::Execute { proposal_id },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    assert!(matches!(err, ContractError::NotPassed {}));

    app.update_block(next_block);

    // close the proposal
    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        proposal_module.clone(),
        &ExecuteMsg::Close { proposal_id },
        &[],
    )
    .unwrap();

    // assert prop is closed and attempt to execute it
    let prop = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(prop.proposal.status, Status::Closed);

    let err = app
        .execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            proposal_module,
            &ExecuteMsg::Execute { proposal_id },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert!(matches!(err, ContractError::NotPassed {}));
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

    app.update_block(next_block);

    // get the proposal to pass
    let vote = MultipleChoiceVote { option_id: 0 };
    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        proposal_module.clone(),
        &ExecuteMsg::Vote {
            proposal_id,
            vote,
            rationale: None,
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        Addr::unchecked(ALTERNATIVE_ADDR),
        proposal_module.clone(),
        &ExecuteMsg::Vote {
            proposal_id,
            vote,
            rationale: None,
        },
        &[],
    )
    .unwrap();

    app.update_block(next_block);

    let prop = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(prop.proposal.status, Status::Passed);

    // execute the proposal
    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        proposal_module.clone(),
        &ExecuteMsg::Execute { proposal_id },
        &[],
    )
    .unwrap();

    app.update_block(next_block);

    let prop = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(prop.proposal.status, Status::Executed);

    let err = app
        .execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            proposal_module,
            &ExecuteMsg::Execute { proposal_id },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert!(matches!(err, ContractError::NotPassed {}));
}

// Users should be able to submit votes past the proposal
// expiration date. Such votes do not affect the outcome
// of the proposals; instead, they are meant to allow
// voters to voice their opinion.
#[test]
pub fn test_allow_voting_after_proposal_execution_pre_expiration_cw20() {
    let mut app = App::default();

    let instantiate = InstantiateMsg {
        voting_strategy: VotingStrategy::SingleChoice {
            quorum: PercentageThreshold::Percent(Decimal::percent(66)),
        },
        max_voting_period: Duration::Time(604800),
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
        veto: None,
        delegation_module: None,
    };

    let core_addr = instantiate_with_multiple_staked_balances_governance(
        &mut app,
        instantiate,
        Some(vec![
            Cw20Coin {
                address: CREATOR_ADDR.to_string(),
                amount: Uint128::new(100_000_000),
            },
            Cw20Coin {
                address: ALTERNATIVE_ADDR.to_string(),
                amount: Uint128::new(50_000_000),
            },
        ]),
    );
    let proposal_module = query_multiple_proposal_module(&app, &core_addr);
    let gov_token = query_dao_token(&app, &core_addr);

    // Mint some tokens to pay the proposal deposit.
    mint_cw20s(&mut app, &gov_token, &core_addr, CREATOR_ADDR, 10_000_000);

    // Option 0 would mint 100_000_000 tokens for CREATOR_ADDR
    let msg = cw20::Cw20ExecuteMsg::Mint {
        recipient: CREATOR_ADDR.to_string(),
        amount: Uint128::new(100_000_000),
    };
    let binary_msg = to_json_binary(&msg).unwrap();

    let options = vec![
        MultipleChoiceOption {
            title: "title 1".to_string(),
            description: "multiple choice option 1".to_string(),
            msgs: vec![WasmMsg::Execute {
                contract_addr: gov_token.to_string(),
                msg: binary_msg,
                funds: vec![],
            }
            .into()],
        },
        MultipleChoiceOption {
            title: "title 2".to_string(),
            description: "multiple choice option 2".to_string(),
            msgs: vec![],
        },
    ];

    let mc_options = MultipleChoiceOptions { options };

    let proposal_id = make_proposal(&mut app, &proposal_module, CREATOR_ADDR, mc_options, None);

    // assert initial CREATOR_ADDR address balance is 0
    let balance = query_balance_cw20(&app, gov_token.to_string(), CREATOR_ADDR);
    assert_eq!(balance, Uint128::zero());

    app.update_block(next_block);

    let vote = MultipleChoiceVote { option_id: 0 };

    // someone votes enough to pass the proposal
    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        proposal_module.clone(),
        &ExecuteMsg::Vote {
            proposal_id,
            vote,
            rationale: None,
        },
        &[],
    )
    .unwrap();

    app.update_block(next_block);

    // assert proposal is passed with expected votes
    let prop = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(prop.proposal.status, Status::Passed);
    assert_eq!(prop.proposal.votes.vote_weights[0], Uint128::new(100000000));
    assert_eq!(prop.proposal.votes.vote_weights[1], Uint128::new(0));

    // someone wakes up and casts their vote to express their
    // opinion (not affecting the result of proposal)
    let vote = MultipleChoiceVote { option_id: 1 };
    app.execute_contract(
        Addr::unchecked(ALTERNATIVE_ADDR),
        proposal_module.clone(),
        &ExecuteMsg::Vote {
            proposal_id,
            vote,
            rationale: None,
        },
        &[],
    )
    .unwrap();

    app.update_block(next_block);

    // assert proposal is passed with expected votes
    let prop = query_proposal(&app, &proposal_module, proposal_id);
    assert_eq!(prop.proposal.status, Status::Passed);
    assert_eq!(prop.proposal.votes.vote_weights[0], Uint128::new(100000000));
    assert_eq!(prop.proposal.votes.vote_weights[1], Uint128::new(50000000));

    // execute the proposal expecting
    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        proposal_module.clone(),
        &ExecuteMsg::Execute { proposal_id: 1 },
        &[],
    )
    .unwrap();

    // assert option 0 message executed as expected changed as expected
    let balance = query_balance_cw20(&app, gov_token.to_string(), CREATOR_ADDR);
    assert_eq!(balance, Uint128::new(110_000_000));
}
