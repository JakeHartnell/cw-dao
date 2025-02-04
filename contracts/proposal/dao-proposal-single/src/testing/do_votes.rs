use std::mem::discriminant;

use cosmwasm_std::{coins, Addr, Coin, Uint128};
use cw20::Cw20Coin;

use cw_multi_test::{App, BankSudo, Executor, SudoMsg};
use dao_interface::state::ProposalModule;
use dao_pre_propose_single as cppbps;

use cw_denom::CheckedDenom;
use dao_testing::{ShouldExecute, TestSingleChoiceVote};
use dao_voting::{
    deposit::{CheckedDepositInfo, UncheckedDepositInfo},
    status::Status,
    threshold::Threshold,
};

use crate::{
    msg::{ExecuteMsg, InstantiateMsg, QueryMsg},
    query::{ProposalResponse, VoteInfo, VoteResponse},
    testing::{instantiate::*, queries::query_deposit_config_and_pre_propose_module},
};

pub(crate) fn do_votes_staked_balances(
    votes: Vec<TestSingleChoiceVote>,
    threshold: Threshold,
    expected_status: Status,
    total_supply: Option<Uint128>,
) {
    do_test_votes(
        votes,
        threshold,
        expected_status,
        total_supply,
        None::<UncheckedDepositInfo>,
        instantiate_with_staked_balances_governance,
    );
}

pub(crate) fn do_votes_nft_balances(
    votes: Vec<TestSingleChoiceVote>,
    threshold: Threshold,
    expected_status: Status,
    total_supply: Option<Uint128>,
) {
    do_test_votes(
        votes,
        threshold,
        expected_status,
        total_supply,
        None,
        instantiate_with_staked_cw721_governance,
    );
}

pub(crate) fn do_votes_native_staked_balances(
    votes: Vec<TestSingleChoiceVote>,
    threshold: Threshold,
    expected_status: Status,
    total_supply: Option<Uint128>,
) {
    do_test_votes(
        votes,
        threshold,
        expected_status,
        total_supply,
        None,
        instantiate_with_native_staked_balances_governance,
    );
}

pub(crate) fn do_votes_cw4_weights(
    votes: Vec<TestSingleChoiceVote>,
    threshold: Threshold,
    expected_status: Status,
    total_supply: Option<Uint128>,
) {
    do_test_votes(
        votes,
        threshold,
        expected_status,
        total_supply,
        None::<UncheckedDepositInfo>,
        instantiate_with_cw4_groups_governance,
    );
}

fn do_test_votes<F>(
    votes: Vec<TestSingleChoiceVote>,
    threshold: Threshold,
    expected_status: Status,
    total_supply: Option<Uint128>,
    deposit_info: Option<UncheckedDepositInfo>,
    setup_governance: F,
) -> (App, Addr)
where
    F: Fn(&mut App, InstantiateMsg, Option<Vec<Cw20Coin>>) -> Addr,
{
    let mut app = App::default();

    // Mint some ujuno so that it exists for native staking tests
    // Otherwise denom validation will fail
    app.sudo(SudoMsg::Bank(BankSudo::Mint {
        to_address: "sodenomexists".to_string(),
        amount: vec![Coin {
            amount: Uint128::new(10),
            denom: "ujuno".to_string(),
        }],
    }))
    .unwrap();

    let mut initial_balances = votes
        .iter()
        .map(|TestSingleChoiceVote { voter, weight, .. }| Cw20Coin {
            address: voter.to_string(),
            amount: *weight,
        })
        .collect::<Vec<Cw20Coin>>();
    let initial_balances_supply = votes.iter().fold(Uint128::zero(), |p, n| p + n.weight);
    let to_fill = total_supply.map(|total_supply| total_supply - initial_balances_supply);
    if let Some(fill) = to_fill {
        initial_balances.push(Cw20Coin {
            address: "filler".to_string(),
            amount: fill,
        })
    }

    let pre_propose_info = get_pre_propose_info(&mut app, deposit_info, false);

    let proposer = match votes.first() {
        Some(vote) => vote.voter.clone(),
        None => panic!("do_test_votes must have at least one vote."),
    };

    let max_voting_period = cw_utils::Duration::Height(6);
    let instantiate = InstantiateMsg {
        veto: None,
        threshold,
        max_voting_period,
        min_voting_period: None,
        only_members_execute: false,
        allow_revoting: false,
        close_proposal_on_execution_failure: true,
        pre_propose_info,
        delegation_module: None,
    };

    let core_addr = setup_governance(&mut app, instantiate, Some(initial_balances));

    let governance_modules: Vec<ProposalModule> = app
        .wrap()
        .query_wasm_smart(
            core_addr.clone(),
            &dao_interface::msg::QueryMsg::ProposalModules {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(governance_modules.len(), 1);
    let proposal_single = governance_modules.into_iter().next().unwrap().address;

    let (deposit_config, pre_propose_module) =
        query_deposit_config_and_pre_propose_module(&app, &proposal_single);
    // Pay the cw20 deposit if needed.
    if let Some(CheckedDepositInfo {
        denom: CheckedDenom::Cw20(ref token),
        amount,
        ..
    }) = deposit_config.deposit_info
    {
        app.execute_contract(
            Addr::unchecked(&proposer),
            token.clone(),
            &cw20_base::msg::ExecuteMsg::IncreaseAllowance {
                spender: pre_propose_module.to_string(),
                amount,
                expires: None,
            },
            &[],
        )
        .unwrap();
    }

    let funds = if let Some(CheckedDepositInfo {
        denom: CheckedDenom::Native(ref denom),
        amount,
        ..
    }) = deposit_config.deposit_info
    {
        // Mint the needed tokens to create the deposit.
        app.sudo(cw_multi_test::SudoMsg::Bank(BankSudo::Mint {
            to_address: proposer.clone(),
            amount: coins(amount.u128(), denom),
        }))
        .unwrap();
        coins(amount.u128(), denom)
    } else {
        vec![]
    };

    app.execute_contract(
        Addr::unchecked(&proposer),
        pre_propose_module,
        &cppbps::ExecuteMsg::Propose {
            msg: cppbps::ProposeMessage::Propose {
                title: "A simple text proposal".to_string(),
                description: "This is a simple text proposal".to_string(),
                msgs: vec![],
                vote: None,
            },
        },
        &funds,
    )
    .unwrap();

    // Cast votes.
    for vote in votes {
        let TestSingleChoiceVote {
            voter,
            position,
            weight,
            should_execute,
        } = vote;
        // Vote on the proposal.
        let res = app.execute_contract(
            Addr::unchecked(voter.clone()),
            proposal_single.clone(),
            &ExecuteMsg::Vote {
                proposal_id: 1,
                vote: position,
                rationale: None,
            },
            &[],
        );
        match should_execute {
            ShouldExecute::Yes => {
                assert!(res.is_ok());
                // Check that the vote was recorded correctly.
                let vote: VoteResponse = app
                    .wrap()
                    .query_wasm_smart(
                        proposal_single.clone(),
                        &QueryMsg::GetVote {
                            proposal_id: 1,
                            voter: voter.clone(),
                        },
                    )
                    .unwrap();
                let expected = VoteResponse {
                    vote: Some(VoteInfo {
                        rationale: None,
                        voter: Addr::unchecked(&voter),
                        vote: position,
                        power: match deposit_config.deposit_info {
                            Some(CheckedDepositInfo {
                                amount,
                                denom: CheckedDenom::Cw20(_),
                                ..
                            }) => {
                                if proposer == voter {
                                    weight - amount
                                } else {
                                    weight
                                }
                            }
                            // Native token deposits shouldn't impact
                            // expected voting power.
                            _ => weight,
                        },
                    }),
                };
                assert_eq!(vote, expected)
            }
            ShouldExecute::No => {
                res.unwrap_err();
            }
            ShouldExecute::Meh => (),
        }
    }

    let proposal: ProposalResponse = app
        .wrap()
        .query_wasm_smart(proposal_single, &QueryMsg::Proposal { proposal_id: 1 })
        .unwrap();

    // We just care about getting the right variant
    assert_eq!(
        discriminant::<Status>(&proposal.proposal.status),
        discriminant::<Status>(&expected_status)
    );

    (app, core_addr)
}

#[test]
fn test_vote_simple() {
    dao_testing::test_simple_votes(do_votes_cw4_weights);
    dao_testing::test_simple_votes(do_votes_staked_balances);
    dao_testing::test_simple_votes(do_votes_nft_balances);
    dao_testing::test_simple_votes(do_votes_native_staked_balances)
}

#[test]
fn test_simple_vote_no_overflow() {
    dao_testing::test_simple_vote_no_overflow(do_votes_staked_balances);
    dao_testing::test_simple_vote_no_overflow(do_votes_native_staked_balances);
}

#[test]
fn test_vote_no_overflow() {
    dao_testing::test_vote_no_overflow(do_votes_staked_balances);
    dao_testing::test_vote_no_overflow(do_votes_native_staked_balances);
}

#[test]
fn test_simple_early_rejection() {
    dao_testing::test_simple_early_rejection(do_votes_cw4_weights);
    dao_testing::test_simple_early_rejection(do_votes_staked_balances);
    dao_testing::test_simple_early_rejection(do_votes_native_staked_balances);
}

#[test]
fn test_vote_abstain_only() {
    dao_testing::test_vote_abstain_only(do_votes_cw4_weights);
    dao_testing::test_vote_abstain_only(do_votes_staked_balances);
    dao_testing::test_vote_abstain_only(do_votes_native_staked_balances);
}

#[test]
fn test_tricky_rounding() {
    dao_testing::test_tricky_rounding(do_votes_cw4_weights);
    dao_testing::test_tricky_rounding(do_votes_staked_balances);
    dao_testing::test_tricky_rounding(do_votes_native_staked_balances);
}

#[test]
fn test_no_double_votes() {
    dao_testing::test_no_double_votes(do_votes_cw4_weights);
    dao_testing::test_no_double_votes(do_votes_staked_balances);
    dao_testing::test_no_double_votes(do_votes_nft_balances);
    dao_testing::test_no_double_votes(do_votes_native_staked_balances);
}

#[test]
fn test_votes_favor_yes() {
    dao_testing::test_votes_favor_yes(do_votes_staked_balances);
    dao_testing::test_votes_favor_yes(do_votes_nft_balances);
    dao_testing::test_votes_favor_yes(do_votes_native_staked_balances);
}

#[test]
fn test_votes_low_threshold() {
    dao_testing::test_votes_low_threshold(do_votes_cw4_weights);
    dao_testing::test_votes_low_threshold(do_votes_staked_balances);
    dao_testing::test_votes_low_threshold(do_votes_nft_balances);
    dao_testing::test_votes_low_threshold(do_votes_native_staked_balances);
}

#[test]
fn test_majority_vs_half() {
    dao_testing::test_majority_vs_half(do_votes_cw4_weights);
    dao_testing::test_majority_vs_half(do_votes_staked_balances);
    dao_testing::test_majority_vs_half(do_votes_nft_balances);
    dao_testing::test_majority_vs_half(do_votes_native_staked_balances);
}

#[test]
fn test_pass_threshold_not_quorum() {
    dao_testing::test_pass_threshold_not_quorum(do_votes_cw4_weights);
    dao_testing::test_pass_threshold_not_quorum(do_votes_staked_balances);
    dao_testing::test_pass_threshold_not_quorum(do_votes_nft_balances);
    dao_testing::test_pass_threshold_not_quorum(do_votes_native_staked_balances);
}

#[test]
fn test_pass_threshold_exactly_quorum() {
    dao_testing::test_pass_exactly_quorum(do_votes_cw4_weights);
    dao_testing::test_pass_exactly_quorum(do_votes_staked_balances);
    dao_testing::test_pass_exactly_quorum(do_votes_nft_balances);
    dao_testing::test_pass_exactly_quorum(do_votes_native_staked_balances);
}

/// Generate some random voting selections and make sure they behave
/// as expected. We split this test up as these take a while and cargo
/// can parallize tests.
#[test]
fn fuzz_voting_cw4_weights() {
    dao_testing::fuzz_voting(do_votes_cw4_weights)
}

#[test]
fn fuzz_voting_staked_balances() {
    dao_testing::fuzz_voting(do_votes_staked_balances)
}

#[test]
fn fuzz_voting_native_staked_balances() {
    dao_testing::fuzz_voting(do_votes_native_staked_balances)
}
