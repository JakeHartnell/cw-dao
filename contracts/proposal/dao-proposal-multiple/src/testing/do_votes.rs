use cosmwasm_std::{coins, Addr, Decimal, Uint128};
use cw20::Cw20Coin;
use cw_denom::CheckedDenom;
use cw_multi_test::{App, BankSudo, Executor};
use dao_interface::state::ProposalModule;
use dao_testing::{contracts::dao_proposal_multiple_contract, ShouldExecute};
use dao_voting::{
    deposit::{CheckedDepositInfo, UncheckedDepositInfo},
    multiple_choice::{
        MultipleChoiceOption, MultipleChoiceOptions, MultipleChoiceVote, VotingStrategy,
    },
    status::Status,
    threshold::PercentageThreshold,
};
use rand::{prelude::SliceRandom, Rng};
use std::panic;

use crate::{
    msg::{ExecuteMsg, InstantiateMsg, QueryMsg},
    query::{ProposalResponse, VoteInfo, VoteResponse},
    testing::{
        instantiate::{
            instantiate_with_cw20_balances_governance, instantiate_with_staked_balances_governance,
        },
        queries::query_deposit_config_and_pre_propose_module,
        tests::{get_pre_propose_info, TestMultipleChoiceVote},
    },
};
use dao_pre_propose_multiple as cppm;

fn do_votes_cw20_balances(
    votes: Vec<TestMultipleChoiceVote>,
    voting_strategy: VotingStrategy,
    expected_status: Status,
    total_supply: Option<Uint128>,
    should_expire: bool,
) {
    do_test_votes(
        votes,
        voting_strategy,
        expected_status,
        total_supply,
        None::<UncheckedDepositInfo>,
        should_expire,
        instantiate_with_staked_balances_governance,
    );
}

fn do_votes_staked_balances(
    votes: Vec<TestMultipleChoiceVote>,
    voting_strategy: VotingStrategy,
    expected_status: Status,
    total_supply: Option<Uint128>,
    should_expire: bool,
) {
    do_test_votes(
        votes,
        voting_strategy,
        expected_status,
        total_supply,
        None::<UncheckedDepositInfo>,
        should_expire,
        instantiate_with_staked_balances_governance,
    );
}

fn do_votes_cw4_weights(
    votes: Vec<TestMultipleChoiceVote>,
    voting_strategy: VotingStrategy,
    expected_status: Status,
    total_supply: Option<Uint128>,
    should_expire: bool,
) {
    do_test_votes(
        votes,
        voting_strategy,
        expected_status,
        total_supply,
        None::<UncheckedDepositInfo>,
        should_expire,
        instantiate_with_staked_balances_governance,
    );
}

// Creates multiple choice proposal with provided config and executes provided votes against it.
fn do_test_votes<F>(
    votes: Vec<TestMultipleChoiceVote>,
    voting_strategy: VotingStrategy,
    expected_status: Status,
    total_supply: Option<Uint128>,
    deposit_info: Option<UncheckedDepositInfo>,
    should_expire: bool,
    setup_governance: F,
) -> (App, Addr)
where
    F: Fn(&mut App, InstantiateMsg, Option<Vec<Cw20Coin>>) -> Addr,
{
    let mut app = App::default();
    let _govmod_id = app.store_code(dao_proposal_multiple_contract());

    let mut initial_balances = votes
        .iter()
        .map(|TestMultipleChoiceVote { voter, weight, .. }| Cw20Coin {
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
        min_voting_period: None,
        max_voting_period,
        only_members_execute: false,
        allow_revoting: false,
        voting_strategy,
        close_proposal_on_execution_failure: true,
        pre_propose_info,
        veto: None,
        delegation_module: None,
    };

    let governance_addr = setup_governance(&mut app, instantiate, Some(initial_balances));

    let governance_modules: Vec<ProposalModule> = app
        .wrap()
        .query_wasm_smart(
            governance_addr.clone(),
            &dao_interface::msg::QueryMsg::ProposalModules {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(governance_modules.len(), 1);
    let govmod = governance_modules.into_iter().next().unwrap().address;

    // Allow a proposal deposit as needed.
    let (deposit_config, pre_propose_module) =
        query_deposit_config_and_pre_propose_module(&app, &govmod);

    // Increase allowance to pay the cw20 deposit if needed.
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

    let options = vec![
        MultipleChoiceOption {
            description: "multiple choice option 1".to_string(),
            msgs: vec![],
            title: "title".to_string(),
        },
        MultipleChoiceOption {
            description: "multiple choice option 2".to_string(),
            msgs: vec![],
            title: "title".to_string(),
        },
    ];

    let mc_options = MultipleChoiceOptions { options };

    app.execute_contract(
        Addr::unchecked(&proposer),
        pre_propose_module,
        &cppm::ExecuteMsg::Propose {
            msg: cppm::ProposeMessage::Propose {
                title: "A simple text proposal".to_string(),
                description: "This is a simple text proposal".to_string(),
                choices: mc_options,
                vote: None,
            },
        },
        &funds,
    )
    .unwrap();

    // Cast votes.
    for vote in votes {
        let TestMultipleChoiceVote {
            voter,
            position,
            weight,
            should_execute,
        } = vote;
        // Vote on the proposal.
        let res = app.execute_contract(
            Addr::unchecked(voter.clone()),
            govmod.clone(),
            &ExecuteMsg::Vote {
                proposal_id: 1,
                vote: position,
                rationale: None,
            },
            &[],
        );
        match should_execute {
            ShouldExecute::Yes => {
                if res.is_err() {
                    panic!()
                }
                // Check that the vote was recorded correctly.
                let vote: VoteResponse = app
                    .wrap()
                    .query_wasm_smart(
                        govmod.clone(),
                        &QueryMsg::GetVote {
                            proposal_id: 1,
                            voter: voter.clone(),
                        },
                    )
                    .unwrap();
                let expected = VoteResponse {
                    vote: Some(VoteInfo {
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
                        rationale: None,
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

    // Expire the proposal if this is expected.
    if should_expire {
        app.update_block(|block| block.height += 100);
    }

    let proposal: ProposalResponse = app
        .wrap()
        .query_wasm_smart(govmod, &QueryMsg::Proposal { proposal_id: 1 })
        .unwrap();

    assert_eq!(proposal.proposal.status, expected_status);

    (app, governance_addr)
}

// Creates a proposal and then executes a series of votes on those
// proposals. Asserts both that those votes execute as expected and
// that the final status of the proposal is what is expected. Returns
// the address of the governance contract that it has created so that
// callers may do additional inspection of the contract's state.
pub fn do_test_votes_cw20_balances(
    votes: Vec<TestMultipleChoiceVote>,
    voting_strategy: VotingStrategy,
    expected_status: Status,
    total_supply: Option<Uint128>,
    deposit_info: Option<UncheckedDepositInfo>,
    should_expire: bool,
) -> (App, Addr) {
    do_test_votes(
        votes,
        voting_strategy,
        expected_status,
        total_supply,
        deposit_info,
        should_expire,
        instantiate_with_cw20_balances_governance,
    )
}

pub fn test_simple_votes<F>(do_test_votes: F)
where
    F: Fn(Vec<TestMultipleChoiceVote>, VotingStrategy, Status, Option<Uint128>, bool),
{
    // Vote for one option, passes
    do_test_votes(
        vec![TestMultipleChoiceVote {
            voter: "bluenote".to_string(),
            position: MultipleChoiceVote { option_id: 0 },
            weight: Uint128::new(10),
            should_execute: ShouldExecute::Yes,
        }],
        VotingStrategy::SingleChoice {
            quorum: PercentageThreshold::Percent(Decimal::percent(100)),
        },
        Status::Passed,
        None,
        false,
    );

    // Vote for none of the above, gets rejected
    do_test_votes(
        vec![TestMultipleChoiceVote {
            voter: "bluenote".to_string(),
            position: MultipleChoiceVote { option_id: 2 },
            weight: Uint128::new(10),
            should_execute: ShouldExecute::Yes,
        }],
        VotingStrategy::SingleChoice {
            quorum: PercentageThreshold::Percent(Decimal::percent(100)),
        },
        Status::Rejected,
        None,
        false,
    )
}

pub fn test_vote_invalid_option<F>(do_test_votes: F)
where
    F: Fn(Vec<TestMultipleChoiceVote>, VotingStrategy, Status, Option<Uint128>, bool),
{
    // Vote for out of bounds option
    do_test_votes(
        vec![TestMultipleChoiceVote {
            voter: "bluenote".to_string(),
            position: MultipleChoiceVote { option_id: 10 },
            weight: Uint128::new(10),
            should_execute: ShouldExecute::No,
        }],
        VotingStrategy::SingleChoice {
            quorum: PercentageThreshold::Percent(Decimal::percent(100)),
        },
        Status::Open,
        None,
        false,
    );
}

pub fn test_vote_no_overflow<F>(do_votes: F)
where
    F: Fn(Vec<TestMultipleChoiceVote>, VotingStrategy, Status, Option<Uint128>, bool),
{
    do_votes(
        vec![TestMultipleChoiceVote {
            voter: "bluenote".to_string(),
            position: MultipleChoiceVote { option_id: 0 },
            weight: Uint128::new(u128::MAX),
            should_execute: ShouldExecute::Yes,
        }],
        VotingStrategy::SingleChoice {
            quorum: PercentageThreshold::Percent(Decimal::percent(100)),
        },
        Status::Passed,
        None,
        false,
    );

    do_votes(
        vec![
            TestMultipleChoiceVote {
                voter: "bluenote".to_string(),
                position: MultipleChoiceVote { option_id: 0 },
                weight: Uint128::new(1),
                should_execute: ShouldExecute::Yes,
            },
            TestMultipleChoiceVote {
                voter: "bob".to_string(),
                position: MultipleChoiceVote { option_id: 1 },
                weight: Uint128::new(u128::MAX - 1),
                should_execute: ShouldExecute::Yes,
            },
        ],
        VotingStrategy::SingleChoice {
            quorum: PercentageThreshold::Percent(Decimal::percent(100)),
        },
        Status::Passed,
        None,
        false,
    );
}

pub fn test_vote_tied_rejected<F>(do_votes: F)
where
    F: Fn(Vec<TestMultipleChoiceVote>, VotingStrategy, Status, Option<Uint128>, bool),
{
    do_votes(
        vec![
            TestMultipleChoiceVote {
                voter: "bluenote".to_string(),
                position: MultipleChoiceVote { option_id: 0 },
                weight: Uint128::new(1),
                should_execute: ShouldExecute::Yes,
            },
            TestMultipleChoiceVote {
                voter: "bob".to_string(),
                position: MultipleChoiceVote { option_id: 1 },
                weight: Uint128::new(1),
                should_execute: ShouldExecute::Yes,
            },
        ],
        VotingStrategy::SingleChoice {
            quorum: PercentageThreshold::Percent(Decimal::percent(100)),
        },
        Status::Rejected,
        None,
        false,
    );
}

pub fn test_vote_none_of_the_above_only<F>(do_votes: F)
where
    F: Fn(Vec<TestMultipleChoiceVote>, VotingStrategy, Status, Option<Uint128>, bool),
{
    do_votes(
        vec![TestMultipleChoiceVote {
            voter: "bluenote".to_string(),
            position: MultipleChoiceVote { option_id: 2 }, // the last index is none of the above
            weight: Uint128::new(u64::MAX.into()),
            should_execute: ShouldExecute::Yes,
        }],
        VotingStrategy::SingleChoice {
            quorum: PercentageThreshold::Percent(Decimal::percent(100)),
        },
        Status::Rejected,
        None,
        false,
    );

    for i in 0..101 {
        do_votes(
            vec![TestMultipleChoiceVote {
                voter: "bluenote".to_string(),
                position: MultipleChoiceVote { option_id: 2 },
                weight: Uint128::new(u64::MAX.into()),
                should_execute: ShouldExecute::Yes,
            }],
            VotingStrategy::SingleChoice {
                quorum: PercentageThreshold::Percent(Decimal::percent(i)),
            },
            Status::Rejected,
            None,
            false,
        );
    }
}

pub fn test_tricky_rounding<F>(do_votes: F)
where
    F: Fn(Vec<TestMultipleChoiceVote>, VotingStrategy, Status, Option<Uint128>, bool),
{
    // This tests the smallest possible round up for passing
    // thresholds we can have. Specifically, a 1% passing threshold
    // and 1 total vote. This should round up and only pass if there
    // are 1 or more yes votes.
    do_votes(
        vec![TestMultipleChoiceVote {
            voter: "bluenote".to_string(),
            position: MultipleChoiceVote { option_id: 0 },
            weight: Uint128::new(1),
            should_execute: ShouldExecute::Yes,
        }],
        VotingStrategy::SingleChoice {
            quorum: PercentageThreshold::Percent(Decimal::percent(1)),
        },
        Status::Passed,
        Some(Uint128::new(100)),
        true,
    );

    do_votes(
        vec![TestMultipleChoiceVote {
            voter: "bluenote".to_string(),
            position: MultipleChoiceVote { option_id: 0 },
            weight: Uint128::new(10),
            should_execute: ShouldExecute::Yes,
        }],
        VotingStrategy::SingleChoice {
            quorum: PercentageThreshold::Percent(Decimal::percent(1)),
        },
        Status::Passed,
        Some(Uint128::new(1000)),
        true,
    );

    // High Precision
    // Proposal should be rejected if < 1% have voted and proposal expires
    do_votes(
        vec![TestMultipleChoiceVote {
            voter: "bluenote".to_string(),
            position: MultipleChoiceVote { option_id: 1 },
            weight: Uint128::new(9999999),
            should_execute: ShouldExecute::Yes,
        }],
        VotingStrategy::SingleChoice {
            quorum: PercentageThreshold::Percent(Decimal::percent(1)),
        },
        Status::Rejected,
        Some(Uint128::new(1000000000)),
        true,
    );

    // Proposal should be rejected if quorum is met but "none of the above" is the winning option.
    do_votes(
        vec![TestMultipleChoiceVote {
            voter: "bluenote".to_string(),
            position: MultipleChoiceVote { option_id: 2 },
            weight: Uint128::new(1),
            should_execute: ShouldExecute::Yes,
        }],
        VotingStrategy::SingleChoice {
            quorum: PercentageThreshold::Percent(Decimal::percent(1)),
        },
        Status::Rejected,
        None,
        false,
    );
}

pub fn test_no_double_votes<F>(do_votes: F)
where
    F: Fn(Vec<TestMultipleChoiceVote>, VotingStrategy, Status, Option<Uint128>, bool),
{
    do_votes(
        vec![
            TestMultipleChoiceVote {
                voter: "bluenote".to_string(),
                position: MultipleChoiceVote { option_id: 1 },
                weight: Uint128::new(2),
                should_execute: ShouldExecute::Yes,
            },
            TestMultipleChoiceVote {
                voter: "bluenote".to_string(),
                position: MultipleChoiceVote { option_id: 1 },
                weight: Uint128::new(2),
                should_execute: ShouldExecute::No,
            },
        ],
        VotingStrategy::SingleChoice {
            quorum: PercentageThreshold::Percent(Decimal::percent(100)),
        },
        // NOTE: Updating our cw20-base version will cause this to
        // fail. In versions of cw20-base before Feb 15 2022 (the one
        // we use at the time of writing) it was allowed to have an
        // initial balance that repeats for a given address but it
        // would cause miscalculation of the total supply. In this
        // case the total supply is miscomputed to be 4 so this is
        // assumed to have 2 abstain votes out of 4 possible votes.
        Status::Open,
        Some(Uint128::new(10)),
        false,
    )
}

pub fn test_majority_vs_half<F>(do_votes: F)
where
    F: Fn(Vec<TestMultipleChoiceVote>, VotingStrategy, Status, Option<Uint128>, bool),
{
    // Half
    do_votes(
        vec![
            TestMultipleChoiceVote {
                voter: "bluenote".to_string(),
                position: MultipleChoiceVote { option_id: 0 },
                weight: Uint128::new(10),
                should_execute: ShouldExecute::Yes,
            },
            TestMultipleChoiceVote {
                voter: "blue".to_string(),
                position: MultipleChoiceVote { option_id: 0 },
                weight: Uint128::new(10),
                should_execute: ShouldExecute::Yes,
            },
        ],
        VotingStrategy::SingleChoice {
            quorum: PercentageThreshold::Percent(Decimal::percent(50)),
        },
        Status::Passed,
        Some(Uint128::new(40)),
        true,
    );

    // Majority
    do_votes(
        vec![
            TestMultipleChoiceVote {
                voter: "bluenote".to_string(),
                position: MultipleChoiceVote { option_id: 0 },
                weight: Uint128::new(10),
                should_execute: ShouldExecute::Yes,
            },
            TestMultipleChoiceVote {
                voter: "blue".to_string(),
                position: MultipleChoiceVote { option_id: 0 },
                weight: Uint128::new(10),
                should_execute: ShouldExecute::Yes,
            },
        ],
        VotingStrategy::SingleChoice {
            quorum: PercentageThreshold::Majority {},
        },
        Status::Rejected,
        Some(Uint128::new(40)),
        true,
    );
}

pub fn test_pass_exactly_quorum<F>(do_votes: F)
where
    F: Fn(Vec<TestMultipleChoiceVote>, VotingStrategy, Status, Option<Uint128>, bool),
{
    do_votes(
        vec![TestMultipleChoiceVote {
            voter: "bluenote".to_string(),
            position: MultipleChoiceVote { option_id: 0 },
            weight: Uint128::new(60),
            should_execute: ShouldExecute::Yes,
        }],
        VotingStrategy::SingleChoice {
            quorum: PercentageThreshold::Percent(Decimal::percent(60)),
        },
        Status::Passed,
        Some(Uint128::new(100)),
        false,
    );

    // None of the above wins
    do_votes(
        vec![TestMultipleChoiceVote {
            voter: "bluenote".to_string(),
            position: MultipleChoiceVote { option_id: 2 },
            weight: Uint128::new(60),
            should_execute: ShouldExecute::Yes,
        }],
        VotingStrategy::SingleChoice {
            quorum: PercentageThreshold::Percent(Decimal::percent(60)),
        },
        Status::Rejected,
        Some(Uint128::new(100)),
        false,
    );
}

pub fn fuzz_voting<F>(do_votes: F)
where
    F: Fn(Vec<TestMultipleChoiceVote>, VotingStrategy, Status, Option<Uint128>, bool),
{
    let mut rng = rand::thread_rng();
    let dist = rand::distributions::Uniform::<u64>::new(1, 200);
    for _ in 0..10 {
        let zero: Vec<u64> = (0..50).map(|_| rng.sample(dist)).collect();
        let one: Vec<u64> = (0..50).map(|_| rng.sample(dist)).collect();
        let none: Vec<u64> = (0..50).map(|_| rng.sample(dist)).collect();

        let zero_sum: u64 = zero.iter().sum();
        let one_sum: u64 = one.iter().sum();
        let none_sum: u64 = none.iter().sum();

        let mut sums = [zero_sum, one_sum, none_sum];
        sums.sort_unstable();

        // If none of the above wins or there is a tie between second and first choice.
        let expected_status: Status = if *sums.last().unwrap() == none_sum || sums[1] == sums[2] {
            Status::Rejected
        } else {
            Status::Passed
        };

        let zero = zero
            .into_iter()
            .enumerate()
            .map(|(idx, weight)| TestMultipleChoiceVote {
                voter: format!("zero_{idx}"),
                position: MultipleChoiceVote { option_id: 0 },
                weight: Uint128::new(weight as u128),
                should_execute: ShouldExecute::Meh,
            });
        let one = one
            .into_iter()
            .enumerate()
            .map(|(idx, weight)| TestMultipleChoiceVote {
                voter: format!("one_{idx}"),
                position: MultipleChoiceVote { option_id: 1 },
                weight: Uint128::new(weight as u128),
                should_execute: ShouldExecute::Meh,
            });

        let none = none
            .into_iter()
            .enumerate()
            .map(|(idx, weight)| TestMultipleChoiceVote {
                voter: format!("none_{idx}"),
                position: MultipleChoiceVote { option_id: 2 },
                weight: Uint128::new(weight as u128),
                should_execute: ShouldExecute::Meh,
            });

        let mut votes = zero.chain(one).chain(none).collect::<Vec<_>>();
        votes.shuffle(&mut rng);

        do_votes(
            votes,
            VotingStrategy::SingleChoice {
                quorum: PercentageThreshold::Majority {},
            },
            expected_status,
            None,
            true,
        );
    }
}

#[test]
fn test_vote_simple() {
    test_simple_votes(do_votes_cw20_balances);
    test_simple_votes(do_votes_cw4_weights);
    test_simple_votes(do_votes_staked_balances)
}

#[test]
fn test_vote_out_of_bounds() {
    test_vote_invalid_option(do_votes_cw20_balances);
    test_vote_invalid_option(do_votes_cw4_weights);
    test_vote_invalid_option(do_votes_staked_balances);
}

#[test]
fn test_no_overflow() {
    test_vote_no_overflow(do_votes_cw20_balances);
    test_vote_no_overflow(do_votes_staked_balances);
    test_vote_no_overflow(do_votes_cw4_weights)
}

#[test]
fn test_quorum_not_met() {
    test_vote_no_overflow(do_votes_cw20_balances);
    test_vote_no_overflow(do_votes_staked_balances);
    test_vote_no_overflow(do_votes_cw4_weights)
}

#[test]
fn test_votes_tied() {
    test_vote_tied_rejected(do_votes_cw20_balances);
    test_vote_tied_rejected(do_votes_staked_balances);
    test_vote_tied_rejected(do_votes_cw4_weights)
}

#[test]
fn test_votes_none_of_the_above() {
    test_vote_none_of_the_above_only(do_votes_cw20_balances);
    test_vote_none_of_the_above_only(do_votes_staked_balances);
    test_vote_none_of_the_above_only(do_votes_cw4_weights)
}

#[test]
fn test_rounding() {
    test_tricky_rounding(do_votes_cw20_balances);
    test_tricky_rounding(do_votes_staked_balances);
    test_tricky_rounding(do_votes_cw4_weights)
}

#[test]
fn test_no_double_vote() {
    test_no_double_votes(do_votes_cw20_balances);
    test_no_double_votes(do_votes_staked_balances);
    test_no_double_votes(do_votes_cw4_weights)
}

#[test]
fn test_majority_half() {
    test_majority_vs_half(do_votes_cw20_balances);
    test_majority_vs_half(do_votes_staked_balances);
    test_majority_vs_half(do_votes_cw4_weights)
}

#[test]
fn test_pass_exact_quorum() {
    test_pass_exactly_quorum(do_votes_cw20_balances);
    test_pass_exactly_quorum(do_votes_staked_balances);
    test_pass_exactly_quorum(do_votes_cw4_weights)
}

#[test]
fn fuzz_votes_cw20_balances() {
    fuzz_voting(do_votes_cw20_balances)
}

#[test]
fn fuzz_votes_cw4_weights() {
    fuzz_voting(do_votes_cw4_weights)
}

#[test]
fn fuzz_votes_staked_balances() {
    fuzz_voting(do_votes_staked_balances)
}
