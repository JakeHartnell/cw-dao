use cosmwasm_std::{
    to_binary, Addr, Binary, CosmosMsg, Decimal, Empty, Timestamp, Uint128, WasmMsg,
};
use cw20::Cw20Coin;
use cw20_staked_balance_voting::msg::ActiveThreshold;
use cw_core::state::ProposalModule;
use cw_multi_test::{next_block, App, Contract, ContractWrapper, Executor};
use cw_utils::Duration;
use indexable_hooks::HooksResponse;
use rand::{prelude::SliceRandom, Rng};
use voting::{
    deposit::{CheckedDepositInfo, DepositInfo, DepositToken},
    status::Status,
    threshold::{PercentageThreshold, Threshold},
    voting::{MultipleChoiceVote, MultipleChoiceVotes},
};

use crate::{
    msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg},
    proposal::MultipleChoiceProposal,
    query::{
        FilterListProposalsResponse, ProposalListResponse, ProposalResponse, VoteListResponse,
        VoteResponse, WalletVote,
    },
    state::{Config, MultipleChoiceOption, MultipleChoiceOptions, VoteInfo, MAX_NUM_CHOICES},
    voting_strategy::VotingStrategy,
    ContractError,
};

use testing::{
    helpers::{
        cw20_balances_voting, cw20_contract, instantiate_with_cw20_balances_governance,
        instantiate_with_staked_balances_governance, instantiate_with_staking_active_threshold,
    },
    ShouldExecute,
};

const CREATOR_ADDR: &str = "creator";

pub struct TestMultipleChoiceVote {
    /// The address casting the vote.
    pub voter: String,
    /// Position on the vote.
    pub position: MultipleChoiceVote,
    /// Voting power of the address.
    pub weight: Uint128,
    /// If this vote is expected to execute.
    pub should_execute: ShouldExecute,
}

fn proposal_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        crate::contract::execute,
        crate::contract::instantiate,
        crate::contract::query,
    )
    .with_reply(crate::contract::reply)
    .with_migrate(crate::contract::migrate);
    Box::new(contract)
}

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
        None::<DepositInfo>,
        should_expire,
        instantiate_with_cw20_balances_governance,
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
        None::<DepositInfo>,
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
        None::<DepositInfo>,
        should_expire,
        instantiate_with_cw20_balances_governance,
    );
}

// Creates multiple choice proposal with provided config and executes provided votes against it.
fn do_test_votes<F>(
    votes: Vec<TestMultipleChoiceVote>,
    voting_strategy: VotingStrategy,
    expected_status: Status,
    total_supply: Option<Uint128>,
    deposit_info: Option<DepositInfo>,
    should_expire: bool,
    setup_governance: F,
) -> (App, Addr)
where
    F: Fn(&mut App, u64, Binary, Option<Vec<Cw20Coin>>) -> Addr,
{
    let mut app = App::default();
    let govmod_id = app.store_code(proposal_contract());

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
        deposit_info,
        voting_strategy,
        close_proposal_on_execution_failure: true,
    };

    let governance_addr = setup_governance(
        &mut app,
        govmod_id,
        to_binary(&instantiate).unwrap(),
        Some(initial_balances),
    );

    let governance_modules: Vec<ProposalModule> = app
        .wrap()
        .query_wasm_smart(
            governance_addr.clone(),
            &cw_core::msg::QueryMsg::ProposalModules {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(governance_modules.len(), 1);
    let govmod = governance_modules.into_iter().next().unwrap().address;

    // Allow a proposal deposit as needed.
    let config: Config = app
        .wrap()
        .query_wasm_smart(govmod.clone(), &QueryMsg::Config {})
        .unwrap();
    if let Some(CheckedDepositInfo {
        ref token, deposit, ..
    }) = config.deposit_info
    {
        app.execute_contract(
            Addr::unchecked(&proposer),
            token.clone(),
            &cw20_base::msg::ExecuteMsg::IncreaseAllowance {
                spender: govmod.to_string(),
                amount: deposit,
                expires: None,
            },
            &[],
        )
        .unwrap();
    }

    let options = vec![
        MultipleChoiceOption {
            description: "multiple choice option 1".to_string(),
            msgs: None,
        },
        MultipleChoiceOption {
            description: "multiple choice option 2".to_string(),
            msgs: None,
        },
    ];

    let mc_options = MultipleChoiceOptions { options };

    app.execute_contract(
        Addr::unchecked(&proposer),
        govmod.clone(),
        &ExecuteMsg::Propose {
            title: "A simple text proposal".to_string(),
            description: "A simple text proposal".to_string(),
            choices: mc_options,
        },
        &[],
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
            },
            &[],
        );
        match should_execute {
            ShouldExecute::Yes => {
                if res.is_err() {
                    println!("{:?}", res.err());
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
                        power: match config.deposit_info {
                            Some(CheckedDepositInfo { deposit, .. }) => {
                                if proposer == voter {
                                    weight - deposit
                                } else {
                                    weight
                                }
                            }
                            None => weight,
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
fn do_test_votes_cw20_balances(
    votes: Vec<TestMultipleChoiceVote>,
    voting_strategy: VotingStrategy,
    expected_status: Status,
    total_supply: Option<Uint128>,
    deposit_info: Option<DepositInfo>,
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
            weight: Uint128::new(u128::max_value()),
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
                weight: Uint128::new(u128::max_value() - 1),
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
            weight: Uint128::new(u64::max_value().into()),
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
                weight: Uint128::new(u64::max_value().into()),
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
        let zero: Vec<u64> = (0..50).map(|_| rng.sample(&dist)).collect();
        let one: Vec<u64> = (0..50).map(|_| rng.sample(&dist)).collect();
        let none: Vec<u64> = (0..50).map(|_| rng.sample(&dist)).collect();

        let zero_sum: u64 = zero.iter().sum();
        let one_sum: u64 = one.iter().sum();
        let none_sum: u64 = none.iter().sum();

        let mut sums = vec![zero_sum, one_sum, none_sum];
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
                voter: format!("zero_{}", idx),
                position: MultipleChoiceVote { option_id: 0 },
                weight: Uint128::new(weight as u128),
                should_execute: ShouldExecute::Meh,
            });
        let one = one
            .into_iter()
            .enumerate()
            .map(|(idx, weight)| TestMultipleChoiceVote {
                voter: format!("one_{}", idx),
                position: MultipleChoiceVote { option_id: 1 },
                weight: Uint128::new(weight as u128),
                should_execute: ShouldExecute::Meh,
            });

        let none = none
            .into_iter()
            .enumerate()
            .map(|(idx, weight)| TestMultipleChoiceVote {
                voter: format!("none_{}", idx),
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
fn test_propose() {
    let mut app = App::default();
    let govmod_id = app.store_code(proposal_contract());

    let max_voting_period = cw_utils::Duration::Height(6);
    let quorum = PercentageThreshold::Majority {};

    let voting_strategy = VotingStrategy::SingleChoice { quorum };

    let instantiate = InstantiateMsg {
        max_voting_period,
        only_members_execute: false,
        allow_revoting: false,
        deposit_info: None,
        voting_strategy: voting_strategy.clone(),
        min_voting_period: None,
        close_proposal_on_execution_failure: true,
    };

    let governance_addr = instantiate_with_cw20_balances_governance(
        &mut app,
        govmod_id,
        to_binary(&instantiate).unwrap(),
        None,
    );

    let governance_modules: Vec<ProposalModule> = app
        .wrap()
        .query_wasm_smart(
            governance_addr.clone(),
            &cw_core::msg::QueryMsg::ProposalModules {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(governance_modules.len(), 1);
    let govmod = governance_modules.into_iter().next().unwrap().address;

    // Check that the config has been configured correctly.
    let config: Config = app
        .wrap()
        .query_wasm_smart(govmod.clone(), &QueryMsg::Config {})
        .unwrap();

    let expected = Config {
        max_voting_period,
        only_members_execute: false,
        allow_revoting: false,
        dao: governance_addr,
        deposit_info: None,
        voting_strategy: voting_strategy.clone(),
        min_voting_period: None,
        close_proposal_on_execution_failure: true,
    };

    assert_eq!(config, expected);

    let options = vec![
        MultipleChoiceOption {
            description: "multiple choice option 1".to_string(),
            msgs: None,
        },
        MultipleChoiceOption {
            description: "multiple choice option 1".to_string(),
            msgs: None,
        },
    ];

    let mc_options = MultipleChoiceOptions { options };
    // Create a new proposal.
    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        govmod.clone(),
        &ExecuteMsg::Propose {
            title: "A simple text proposal".to_string(),
            description: "A simple text proposal".to_string(),
            choices: mc_options.clone(),
        },
        &[],
    )
    .unwrap();

    let created: ProposalResponse = app
        .wrap()
        .query_wasm_smart(govmod, &QueryMsg::Proposal { proposal_id: 1 })
        .unwrap();

    let current_block = app.block_info();
    let checked_options = mc_options.into_checked().unwrap();
    let expected = MultipleChoiceProposal {
        title: "A simple text proposal".to_string(),
        description: "A simple text proposal".to_string(),
        proposer: Addr::unchecked(CREATOR_ADDR),
        start_height: current_block.height,
        expiration: max_voting_period.after(&current_block),
        choices: checked_options.options,
        status: Status::Open,
        voting_strategy,
        total_power: Uint128::new(100_000_000),
        votes: MultipleChoiceVotes {
            vote_weights: vec![Uint128::zero(); 3],
        },
        allow_revoting: false,
        deposit_info: None,
        min_voting_period: None,
        created: current_block.time,
        last_updated: current_block.time,
    };

    assert_eq!(created.proposal, expected);
    assert_eq!(created.id, 1u64);
}

#[test]
fn test_propose_wrong_num_choices() {
    let mut app = App::default();
    let govmod_id = app.store_code(proposal_contract());

    let max_voting_period = cw_utils::Duration::Height(6);
    let quorum = PercentageThreshold::Majority {};

    let voting_strategy = VotingStrategy::SingleChoice { quorum };

    let instantiate = InstantiateMsg {
        min_voting_period: None,
        close_proposal_on_execution_failure: true,
        max_voting_period,
        only_members_execute: false,
        allow_revoting: false,
        deposit_info: None,
        voting_strategy: voting_strategy.clone(),
    };

    let governance_addr = instantiate_with_cw20_balances_governance(
        &mut app,
        govmod_id,
        to_binary(&instantiate).unwrap(),
        None,
    );

    let governance_modules: Vec<ProposalModule> = app
        .wrap()
        .query_wasm_smart(
            governance_addr.clone(),
            &cw_core::msg::QueryMsg::ProposalModules {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(governance_modules.len(), 1);
    let govmod = governance_modules.into_iter().next().unwrap().address;

    // Check that the config has been configured correctly.
    let config: Config = app
        .wrap()
        .query_wasm_smart(govmod.clone(), &QueryMsg::Config {})
        .unwrap();

    let expected = Config {
        min_voting_period: None,
        close_proposal_on_execution_failure: true,
        max_voting_period,
        only_members_execute: false,
        allow_revoting: false,
        dao: governance_addr,
        deposit_info: None,
        voting_strategy,
    };

    assert_eq!(config, expected);

    let options = vec![];

    // Create a proposal with less than min choices.
    let mc_options = MultipleChoiceOptions { options };
    let err = app
        .execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            govmod.clone(),
            &ExecuteMsg::Propose {
                title: "A simple text proposal".to_string(),
                description: "A simple text proposal".to_string(),
                choices: mc_options,
            },
            &[],
        )
        .unwrap_err();

    assert!(matches!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::WrongNumberOfChoices {}
    ));

    let options = vec![
        MultipleChoiceOption {
            description: "multiple choice option 1".to_string(),
            msgs: None,
        };
        std::convert::TryInto::try_into(MAX_NUM_CHOICES + 1).unwrap()
    ];

    // Create proposal with more than max choices.

    let mc_options = MultipleChoiceOptions { options };
    // Create a new proposal.
    let err = app
        .execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            govmod,
            &ExecuteMsg::Propose {
                title: "A simple text proposal".to_string(),
                description: "A simple text proposal".to_string(),
                choices: mc_options,
            },
            &[],
        )
        .unwrap_err();

    assert!(matches!(
        err.downcast::<ContractError>().unwrap(),
        ContractError::WrongNumberOfChoices {}
    ));
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

#[test]
fn test_migrate() {
    let mut app = App::default();
    let govmod_id = app.store_code(proposal_contract());

    let msg = InstantiateMsg {
        voting_strategy: VotingStrategy::SingleChoice {
            quorum: PercentageThreshold::Percent(Decimal::percent(10)),
        },
        max_voting_period: Duration::Time(10),
        min_voting_period: None,
        close_proposal_on_execution_failure: true,
        only_members_execute: true,
        allow_revoting: false,
        deposit_info: None,
    };

    let governance_addr = instantiate_with_cw20_balances_governance(
        &mut app,
        govmod_id,
        to_binary(&msg).unwrap(),
        None,
    );
    let governance_modules: Vec<ProposalModule> = app
        .wrap()
        .query_wasm_smart(
            governance_addr.clone(),
            &cw_core::msg::QueryMsg::ProposalModules {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(governance_modules.len(), 1);
    let govmod = governance_modules.into_iter().next().unwrap().address;

    let config: Config = app
        .wrap()
        .query_wasm_smart(govmod.clone(), &QueryMsg::Config {})
        .unwrap();

    app.execute(
        governance_addr,
        CosmosMsg::Wasm(WasmMsg::Migrate {
            contract_addr: govmod.to_string(),
            new_code_id: govmod_id,
            msg: to_binary(&MigrateMsg {}).unwrap(),
        }),
    )
    .unwrap();

    let new_config: Config = app
        .wrap()
        .query_wasm_smart(govmod, &QueryMsg::Config {})
        .unwrap();

    assert_eq!(config, new_config);
}

#[test]
fn test_proposal_count_initialized_to_zero() {
    let mut app = App::default();
    let proposal_id = app.store_code(proposal_contract());
    let msg = InstantiateMsg {
        voting_strategy: VotingStrategy::SingleChoice {
            quorum: PercentageThreshold::Percent(Decimal::percent(10)),
        },
        max_voting_period: Duration::Height(10),
        min_voting_period: None,
        close_proposal_on_execution_failure: true,
        only_members_execute: true,
        allow_revoting: false,
        deposit_info: None,
    };
    let core_addr = instantiate_with_staked_balances_governance(
        &mut app,
        proposal_id,
        to_binary(&msg).unwrap(),
        None,
    );

    let gov_state: cw_core::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(core_addr, &cw_core::msg::QueryMsg::DumpState {})
        .unwrap();
    let proposal_modules = gov_state.proposal_modules;

    assert_eq!(proposal_modules.len(), 1);
    let govmod = proposal_modules.into_iter().next().unwrap().address;

    let proposal_count: u64 = app
        .wrap()
        .query_wasm_smart(govmod, &QueryMsg::ProposalCount {})
        .unwrap();

    assert_eq!(proposal_count, 0);
}

#[test]
fn test_no_early_pass_with_min_duration() {
    let mut app = App::default();
    let govmod_id = app.store_code(proposal_contract());
    let msg = InstantiateMsg {
        voting_strategy: VotingStrategy::SingleChoice {
            quorum: PercentageThreshold::Percent(Decimal::percent(10)),
        },
        max_voting_period: Duration::Height(10),
        min_voting_period: Some(Duration::Height(2)),
        only_members_execute: true,
        allow_revoting: false,
        deposit_info: None,
        close_proposal_on_execution_failure: true,
    };

    let core_addr = instantiate_with_staked_balances_governance(
        &mut app,
        govmod_id,
        to_binary(&msg).unwrap(),
        Some(vec![
            Cw20Coin {
                address: "blue".to_string(),
                amount: Uint128::new(10),
            },
            Cw20Coin {
                address: "whale".to_string(),
                amount: Uint128::new(90),
            },
        ]),
    );

    let gov_state: cw_core::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(core_addr, &cw_core::msg::QueryMsg::DumpState {})
        .unwrap();
    let proposal_modules = gov_state.proposal_modules;

    assert_eq!(proposal_modules.len(), 1);
    let govmod = proposal_modules.into_iter().next().unwrap().address;

    let options = vec![
        MultipleChoiceOption {
            description: "multiple choice option 1".to_string(),
            msgs: None,
        },
        MultipleChoiceOption {
            description: "multiple choice option 2".to_string(),
            msgs: None,
        },
    ];

    let mc_options = MultipleChoiceOptions { options };

    app.execute_contract(
        Addr::unchecked("whale"),
        govmod.clone(),
        &ExecuteMsg::Propose {
            title: "A simple text proposal".to_string(),
            description: "This is a simple text proposal".to_string(),
            choices: mc_options,
        },
        &[],
    )
    .unwrap();

    // Whale votes which under normal curcumstances would cause the
    // proposal to pass. Because there is a min duration it does not.
    app.execute_contract(
        Addr::unchecked("whale"),
        govmod.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 1,
            vote: MultipleChoiceVote { option_id: 0 },
        },
        &[],
    )
    .unwrap();

    let proposal: ProposalResponse = app
        .wrap()
        .query_wasm_smart(govmod.clone(), &QueryMsg::Proposal { proposal_id: 1 })
        .unwrap();

    assert_eq!(proposal.proposal.status, Status::Open);

    // Let the min voting period pass.
    app.update_block(|b| b.height += 2);

    let proposal: ProposalResponse = app
        .wrap()
        .query_wasm_smart(govmod, &QueryMsg::Proposal { proposal_id: 1 })
        .unwrap();

    assert_eq!(proposal.proposal.status, Status::Passed);
}

#[test]
fn test_propose_with_messages() {
    let mut app = App::default();
    let govmod_id = app.store_code(proposal_contract());
    let msg = InstantiateMsg {
        voting_strategy: VotingStrategy::SingleChoice {
            quorum: PercentageThreshold::Percent(Decimal::percent(10)),
        },
        max_voting_period: Duration::Height(10),
        min_voting_period: None,
        close_proposal_on_execution_failure: true,
        only_members_execute: true,
        allow_revoting: false,
        deposit_info: None,
    };

    let core_addr = instantiate_with_staked_balances_governance(
        &mut app,
        govmod_id,
        to_binary(&msg).unwrap(),
        Some(vec![
            Cw20Coin {
                address: "blue".to_string(),
                amount: Uint128::new(10),
            },
            Cw20Coin {
                address: "whale".to_string(),
                amount: Uint128::new(90),
            },
        ]),
    );

    let gov_state: cw_core::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(core_addr, &cw_core::msg::QueryMsg::DumpState {})
        .unwrap();
    let proposal_modules = gov_state.proposal_modules;

    assert_eq!(proposal_modules.len(), 1);
    let govmod = proposal_modules.into_iter().next().unwrap().address;

    let config_msg = ExecuteMsg::UpdateConfig {
        voting_strategy: VotingStrategy::SingleChoice {
            quorum: PercentageThreshold::Majority {},
        },
        min_voting_period: None,
        close_proposal_on_execution_failure: true,
        max_voting_period: cw_utils::Duration::Height(20),
        only_members_execute: false,
        allow_revoting: false,
        dao: "dao".to_string(),
        deposit_info: None,
    };

    let wasm_msg = WasmMsg::Execute {
        contract_addr: govmod.to_string(),
        msg: to_binary(&config_msg).unwrap(),
        funds: vec![],
    };

    let options = vec![
        MultipleChoiceOption {
            description: "multiple choice option 1".to_string(),
            msgs: Some(vec![CosmosMsg::Wasm(wasm_msg)]),
        },
        MultipleChoiceOption {
            description: "multiple choice option 2".to_string(),
            msgs: None,
        },
    ];

    let mc_options = MultipleChoiceOptions { options };

    app.execute_contract(
        Addr::unchecked("whale"),
        govmod.clone(),
        &ExecuteMsg::Propose {
            title: "A simple text proposal".to_string(),
            description: "This is a simple text proposal".to_string(),
            choices: mc_options,
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        Addr::unchecked("whale"),
        govmod.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 1,
            vote: MultipleChoiceVote { option_id: 0 },
        },
        &[],
    )
    .unwrap();

    let proposal: ProposalResponse = app
        .wrap()
        .query_wasm_smart(govmod.clone(), &QueryMsg::Proposal { proposal_id: 1 })
        .unwrap();

    assert_eq!(proposal.proposal.status, Status::Passed);

    // Execute the proposal and messages
    app.execute_contract(
        Addr::unchecked("blue"),
        govmod.clone(),
        &ExecuteMsg::Execute { proposal_id: 1 },
        &[],
    )
    .unwrap();

    // Check that config was updated by proposal message
    let config: Config = app
        .wrap()
        .query_wasm_smart(govmod, &QueryMsg::Config {})
        .unwrap();
    assert_eq!(config.max_voting_period, Duration::Height(20))
}

#[test]
#[should_panic(
    expected = "min_voting_period and max_voting_period must have the same units (height or time)"
)]
fn test_min_duration_units_missmatch() {
    let mut app = App::default();
    let govmod_id = app.store_code(proposal_contract());
    let msg = InstantiateMsg {
        voting_strategy: VotingStrategy::SingleChoice {
            quorum: PercentageThreshold::Percent(Decimal::percent(10)),
        },
        max_voting_period: Duration::Height(10),
        min_voting_period: Some(Duration::Time(2)),
        only_members_execute: true,
        allow_revoting: false,
        deposit_info: None,
        close_proposal_on_execution_failure: true,
    };
    instantiate_with_staked_balances_governance(
        &mut app,
        govmod_id,
        to_binary(&msg).unwrap(),
        Some(vec![
            Cw20Coin {
                address: "blue".to_string(),
                amount: Uint128::new(10),
            },
            Cw20Coin {
                address: "wale".to_string(),
                amount: Uint128::new(90),
            },
        ]),
    );
}

#[test]
#[should_panic(expected = "Min voting period must be less than or equal to max voting period")]
fn test_min_duration_larger_than_proposal_duration() {
    let mut app = App::default();
    let govmod_id = app.store_code(proposal_contract());
    let msg = InstantiateMsg {
        voting_strategy: VotingStrategy::SingleChoice {
            quorum: PercentageThreshold::Percent(Decimal::percent(10)),
        },
        max_voting_period: Duration::Height(10),
        min_voting_period: Some(Duration::Height(11)),
        only_members_execute: true,
        allow_revoting: false,
        deposit_info: None,
        close_proposal_on_execution_failure: true,
    };
    instantiate_with_staked_balances_governance(
        &mut app,
        govmod_id,
        to_binary(&msg).unwrap(),
        Some(vec![
            Cw20Coin {
                address: "blue".to_string(),
                amount: Uint128::new(10),
            },
            Cw20Coin {
                address: "wale".to_string(),
                amount: Uint128::new(90),
            },
        ]),
    );
}

#[test]
fn test_min_duration_same_as_proposal_duration() {
    let mut app = App::default();
    let govmod_id = app.store_code(proposal_contract());
    let msg = InstantiateMsg {
        voting_strategy: VotingStrategy::SingleChoice {
            quorum: PercentageThreshold::Percent(Decimal::percent(10)),
        },
        max_voting_period: Duration::Time(10),
        min_voting_period: Some(Duration::Time(10)),
        only_members_execute: true,
        allow_revoting: false,
        deposit_info: None,
        close_proposal_on_execution_failure: true,
    };

    let core_addr = instantiate_with_staked_balances_governance(
        &mut app,
        govmod_id,
        to_binary(&msg).unwrap(),
        Some(vec![
            Cw20Coin {
                address: "blue".to_string(),
                amount: Uint128::new(10),
            },
            Cw20Coin {
                address: "whale".to_string(),
                amount: Uint128::new(90),
            },
        ]),
    );

    let gov_state: cw_core::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(core_addr, &cw_core::msg::QueryMsg::DumpState {})
        .unwrap();
    let proposal_modules = gov_state.proposal_modules;

    assert_eq!(proposal_modules.len(), 1);
    let govmod = proposal_modules.into_iter().next().unwrap().address;

    let options = vec![
        MultipleChoiceOption {
            description: "multiple choice option 1".to_string(),
            msgs: None,
        },
        MultipleChoiceOption {
            description: "multiple choice option 2".to_string(),
            msgs: None,
        },
    ];

    let mc_options = MultipleChoiceOptions { options };

    app.execute_contract(
        Addr::unchecked("whale"),
        govmod.clone(),
        &ExecuteMsg::Propose {
            title: "A simple text proposal".to_string(),
            description: "This is a simple text proposal".to_string(),
            choices: mc_options,
        },
        &[],
    )
    .unwrap();

    // Whale votes which under normal curcumstances would cause the
    // proposal to pass. Because there is a min duration it does not.
    app.execute_contract(
        Addr::unchecked("whale"),
        govmod.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 1,
            vote: MultipleChoiceVote { option_id: 0 },
        },
        &[],
    )
    .unwrap();

    let proposal: ProposalResponse = app
        .wrap()
        .query_wasm_smart(govmod.clone(), &QueryMsg::Proposal { proposal_id: 1 })
        .unwrap();

    assert_eq!(proposal.proposal.status, Status::Open);

    // someone else can vote none of the above.
    app.execute_contract(
        Addr::unchecked("blue"),
        govmod.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 1,
            vote: MultipleChoiceVote { option_id: 2 },
        },
        &[],
    )
    .unwrap();

    // Let the min voting period pass.
    app.update_block(|b| b.time = b.time.plus_seconds(10));

    let proposal: ProposalResponse = app
        .wrap()
        .query_wasm_smart(govmod, &QueryMsg::Proposal { proposal_id: 1 })
        .unwrap();

    assert_eq!(proposal.proposal.status, Status::Passed);
}

/// Instantiate the contract and use the voting module's token
/// contract as the proposal deposit token.
#[test]
fn test_voting_module_token_proposal_deposit_instantiate() {
    let mut app = App::default();
    let govmod_id = app.store_code(proposal_contract());

    let quorum = PercentageThreshold::Majority {};
    let voting_strategy = VotingStrategy::SingleChoice { quorum };
    let max_voting_period = cw_utils::Duration::Height(6);
    let deposit_info = Some(DepositInfo {
        token: DepositToken::VotingModuleToken {},
        deposit: Uint128::new(1),
        refund_failed_proposals: true,
    });

    let instantiate = InstantiateMsg {
        min_voting_period: None,
        close_proposal_on_execution_failure: true,
        max_voting_period,
        only_members_execute: false,
        allow_revoting: false,
        deposit_info,
        voting_strategy,
    };

    let governance_addr = instantiate_with_cw20_balances_governance(
        &mut app,
        govmod_id,
        to_binary(&instantiate).unwrap(),
        None,
    );

    let gov_state: cw_core::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(governance_addr, &cw_core::msg::QueryMsg::DumpState {})
        .unwrap();
    let governance_modules = gov_state.proposal_modules;
    let voting_module = gov_state.voting_module;

    assert_eq!(governance_modules.len(), 1);
    let govmod = governance_modules.into_iter().next().unwrap().address;

    let config: Config = app
        .wrap()
        .query_wasm_smart(govmod, &QueryMsg::Config {})
        .unwrap();
    let expected_token: Addr = app
        .wrap()
        .query_wasm_smart(
            voting_module,
            &cw_core_interface::voting::Query::TokenContract {},
        )
        .unwrap();

    assert_eq!(
        config.deposit_info,
        Some(CheckedDepositInfo {
            token: expected_token,
            deposit: Uint128::new(1),
            refund_failed_proposals: true
        })
    )
}

// Instantiate the contract and use a cw20 unrealated to the voting
// module for the proposal deposit.
#[test]
fn test_different_token_proposal_deposit() {
    let mut app = App::default();
    let govmod_id = app.store_code(proposal_contract());
    let cw20_id = app.store_code(cw20_contract());
    let cw20_addr = app
        .instantiate_contract(
            cw20_id,
            Addr::unchecked(CREATOR_ADDR),
            &cw20_base::msg::InstantiateMsg {
                name: "OAD OAD".to_string(),
                symbol: "OAD".to_string(),
                decimals: 6,
                initial_balances: vec![],
                mint: None,
                marketing: None,
            },
            &[],
            "random-cw20",
            None,
        )
        .unwrap();

    let quorum = PercentageThreshold::Percent(Decimal::percent(10));
    let voting_strategy = VotingStrategy::SingleChoice { quorum };
    let max_voting_period = cw_utils::Duration::Height(6);
    let instantiate = InstantiateMsg {
        min_voting_period: None,
        close_proposal_on_execution_failure: true,
        max_voting_period,
        only_members_execute: false,
        allow_revoting: false,
        deposit_info: Some(DepositInfo {
            token: DepositToken::Token {
                address: cw20_addr.to_string(),
            },
            deposit: Uint128::new(1),
            refund_failed_proposals: true,
        }),
        voting_strategy,
    };

    instantiate_with_cw20_balances_governance(
        &mut app,
        govmod_id,
        to_binary(&instantiate).unwrap(),
        None,
    );
}

/// Try to instantiate the governance module with a non-cw20 as its
/// proposal deposit token. This should error as the `TokenInfo {}`
/// query ought to fail.
#[test]
#[should_panic(expected = "Error parsing into type cw20_balance_voting::msg::QueryMsg")]
fn test_bad_token_proposal_deposit() {
    let mut app = App::default();
    let govmod_id = app.store_code(proposal_contract());
    let cw20_id = app.store_code(cw20_contract());
    let votemod_id = app.store_code(cw20_balances_voting());

    let votemod_addr = app
        .instantiate_contract(
            votemod_id,
            Addr::unchecked(CREATOR_ADDR),
            &cw20_balance_voting::msg::InstantiateMsg {
                token_info: cw20_balance_voting::msg::TokenInfo::New {
                    code_id: cw20_id,
                    label: "DAO DAO governance token".to_string(),
                    name: "DAO".to_string(),
                    symbol: "DAO".to_string(),
                    decimals: 6,
                    initial_balances: vec![Cw20Coin {
                        address: CREATOR_ADDR.to_string(),
                        amount: Uint128::new(1),
                    }],
                    marketing: None,
                },
            },
            &[],
            "random-vote-module",
            None,
        )
        .unwrap();

    let deposit_info = Some(DepositInfo {
        token: DepositToken::Token {
            address: votemod_addr.to_string(),
        },
        deposit: Uint128::new(1),
        refund_failed_proposals: true,
    });

    let quorum = PercentageThreshold::Percent(Decimal::percent(10));
    let voting_strategy = VotingStrategy::SingleChoice { quorum };
    let max_voting_period = cw_utils::Duration::Height(6);
    let instantiate = InstantiateMsg {
        min_voting_period: None,
        close_proposal_on_execution_failure: true,
        max_voting_period,
        only_members_execute: false,
        allow_revoting: false,
        deposit_info,
        voting_strategy,
    };

    instantiate_with_cw20_balances_governance(
        &mut app,
        govmod_id,
        to_binary(&instantiate).unwrap(),
        None,
    );
}

#[test]
fn test_take_proposal_deposit() {
    let mut app = App::default();
    let govmod_id = app.store_code(proposal_contract());

    let quorum = PercentageThreshold::Percent(Decimal::percent(10));
    let voting_strategy = VotingStrategy::SingleChoice { quorum };
    let max_voting_period = cw_utils::Duration::Height(6);
    let deposit_info = Some(DepositInfo {
        token: DepositToken::VotingModuleToken {},
        deposit: Uint128::new(1),
        refund_failed_proposals: true,
    });

    let instantiate = InstantiateMsg {
        min_voting_period: None,
        close_proposal_on_execution_failure: true,
        max_voting_period,
        only_members_execute: false,
        allow_revoting: false,
        deposit_info,
        voting_strategy,
    };

    let governance_addr = instantiate_with_cw20_balances_governance(
        &mut app,
        govmod_id,
        to_binary(&instantiate).unwrap(),
        Some(vec![Cw20Coin {
            address: "blue".to_string(),
            amount: Uint128::new(2),
        }]),
    );

    let gov_state: cw_core::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(governance_addr, &cw_core::msg::QueryMsg::DumpState {})
        .unwrap();
    let governance_modules = gov_state.proposal_modules;

    assert_eq!(governance_modules.len(), 1);
    let govmod = governance_modules.into_iter().next().unwrap().address;

    let govmod_config: Config = app
        .wrap()
        .query_wasm_smart(govmod.clone(), &QueryMsg::Config {})
        .unwrap();
    let CheckedDepositInfo {
        token,
        deposit,
        refund_failed_proposals,
    } = govmod_config.deposit_info.unwrap();
    assert!(refund_failed_proposals);
    assert_eq!(deposit, Uint128::new(1));

    let options = vec![
        MultipleChoiceOption {
            description: "multiple choice option 1".to_string(),
            msgs: None,
        },
        MultipleChoiceOption {
            description: "multiple choice option 2".to_string(),
            msgs: None,
        },
    ];

    let mc_options = MultipleChoiceOptions { options };

    // This should fail because we have not created an allowance for
    // the proposal deposit.
    app.execute_contract(
        Addr::unchecked("blue"),
        govmod.clone(),
        &ExecuteMsg::Propose {
            title: "A simple text proposal".to_string(),
            description: "This is a simple text proposal".to_string(),
            choices: mc_options.clone(),
        },
        &[],
    )
    .unwrap_err();

    // Allow a proposal deposit.
    app.execute_contract(
        Addr::unchecked("blue"),
        token.clone(),
        &cw20_base::msg::ExecuteMsg::IncreaseAllowance {
            spender: govmod.to_string(),
            amount: Uint128::new(1),
            expires: None,
        },
        &[],
    )
    .unwrap();

    // Now we can create a proposal.
    app.execute_contract(
        Addr::unchecked("blue"),
        govmod,
        &ExecuteMsg::Propose {
            title: "A simple text proposal".to_string(),
            description: "This is a simple text proposal".to_string(),
            choices: mc_options,
        },
        &[],
    )
    .unwrap();

    // Check that our balance was deducted.
    let balance: cw20::BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            token,
            &cw20::Cw20QueryMsg::Balance {
                address: "blue".to_string(),
            },
        )
        .unwrap();
    assert_eq!(balance.balance, Uint128::new(1))
}

#[test]
fn test_deposit_return_on_execute() {
    // Will create a proposal and execute it, one token will be
    // deposited to create said proposal, expectation is that the
    // token is then returned once the proposal is executed.
    let (mut app, governance_addr) = do_test_votes_cw20_balances(
        vec![TestMultipleChoiceVote {
            voter: "blue".to_string(),
            position: MultipleChoiceVote { option_id: 0 },
            weight: Uint128::new(10),
            should_execute: ShouldExecute::Yes,
        }],
        VotingStrategy::SingleChoice {
            quorum: PercentageThreshold::Majority {},
        },
        Status::Passed,
        None,
        Some(DepositInfo {
            token: DepositToken::VotingModuleToken {},
            deposit: Uint128::new(1),
            refund_failed_proposals: false,
        }),
        true,
    );

    let gov_state: cw_core::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(governance_addr, &cw_core::msg::QueryMsg::DumpState {})
        .unwrap();
    let governance_modules = gov_state.proposal_modules;

    assert_eq!(governance_modules.len(), 1);
    let govmod = governance_modules.into_iter().next().unwrap().address;

    let govmod_config: Config = app
        .wrap()
        .query_wasm_smart(govmod.clone(), &QueryMsg::Config {})
        .unwrap();
    let CheckedDepositInfo { token, .. } = govmod_config.deposit_info.unwrap();
    let balance: cw20::BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            token.clone(),
            &cw20::Cw20QueryMsg::Balance {
                address: "blue".to_string(),
            },
        )
        .unwrap();

    // Proposal has not been executed so deposit has not been
    // refunded.
    assert_eq!(balance.balance, Uint128::new(9));

    // Execute the proposal, this should cause the deposit to be
    // refunded.
    app.execute_contract(
        Addr::unchecked("blue"),
        govmod,
        &ExecuteMsg::Execute { proposal_id: 1 },
        &[],
    )
    .unwrap();

    let balance: cw20::BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            token,
            &cw20::Cw20QueryMsg::Balance {
                address: "blue".to_string(),
            },
        )
        .unwrap();

    // Proposal has been executed so deposit has been refunded.
    assert_eq!(balance.balance, Uint128::new(10));
}

#[test]
fn test_deposit_return_zero() {
    // Test that balance does not change when deposit is zero.
    let deposit_info = Some(DepositInfo {
        token: DepositToken::VotingModuleToken {},
        deposit: Uint128::new(0),
        refund_failed_proposals: false,
    });

    let (mut app, governance_addr) = do_test_votes_cw20_balances(
        vec![TestMultipleChoiceVote {
            voter: "blue".to_string(),
            position: MultipleChoiceVote { option_id: 0 },
            weight: Uint128::new(10),
            should_execute: ShouldExecute::Yes,
        }],
        VotingStrategy::SingleChoice {
            quorum: PercentageThreshold::Majority {},
        },
        Status::Passed,
        None,
        deposit_info,
        true,
    );

    let gov_state: cw_core::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(governance_addr, &cw_core::msg::QueryMsg::DumpState {})
        .unwrap();
    let governance_modules = gov_state.proposal_modules;

    assert_eq!(governance_modules.len(), 1);
    let govmod = governance_modules.into_iter().next().unwrap().address;

    let govmod_config: Config = app
        .wrap()
        .query_wasm_smart(govmod.clone(), &QueryMsg::Config {})
        .unwrap();
    let CheckedDepositInfo { token, .. } = govmod_config.deposit_info.unwrap();

    // Execute the proposal
    app.execute_contract(
        Addr::unchecked("blue"),
        govmod,
        &ExecuteMsg::Execute { proposal_id: 1 },
        &[],
    )
    .unwrap();

    let balance: cw20::BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            token,
            &cw20::Cw20QueryMsg::Balance {
                address: "blue".to_string(),
            },
        )
        .unwrap();

    // Proposal has been executed so deposit has been 'refunded'.
    assert_eq!(balance.balance, Uint128::new(10));
}

#[test]
fn test_query_list_votes() {
    let (app, governance_addr) = do_test_votes_cw20_balances(
        vec![
            TestMultipleChoiceVote {
                voter: "blue".to_string(),
                position: MultipleChoiceVote { option_id: 0 },
                weight: Uint128::new(10),
                should_execute: ShouldExecute::Yes,
            },
            TestMultipleChoiceVote {
                voter: "note".to_string(),
                position: MultipleChoiceVote { option_id: 1 },
                weight: Uint128::new(20),
                should_execute: ShouldExecute::Yes,
            },
        ],
        VotingStrategy::SingleChoice {
            quorum: PercentageThreshold::Majority {},
        },
        Status::Passed,
        None,
        None,
        true,
    );

    let gov_state: cw_core::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(governance_addr, &cw_core::msg::QueryMsg::DumpState {})
        .unwrap();
    let governance_modules = gov_state.proposal_modules;

    assert_eq!(governance_modules.len(), 1);
    let govmod = governance_modules.into_iter().next().unwrap().address;

    let list_votes: VoteListResponse = app
        .wrap()
        .query_wasm_smart(
            govmod,
            &QueryMsg::ListVotes {
                proposal_id: 1,
                start_after: None,
                limit: None,
            },
        )
        .unwrap();

    let expected = vec![
        VoteInfo {
            voter: Addr::unchecked("blue"),
            vote: MultipleChoiceVote { option_id: 0 },
            power: Uint128::new(10),
        },
        VoteInfo {
            voter: Addr::unchecked("note"),
            vote: MultipleChoiceVote { option_id: 1 },
            power: Uint128::new(20),
        },
    ];

    assert_eq!(list_votes.votes, expected)
}

#[test]
fn test_invalid_quorum() {
    // Create a proposal that will be rejected
    let (_app, _governance_addr) = do_test_votes_cw20_balances(
        vec![TestMultipleChoiceVote {
            voter: "blue".to_string(),
            position: MultipleChoiceVote { option_id: 2 },
            weight: Uint128::new(10),
            should_execute: ShouldExecute::Yes,
        }],
        VotingStrategy::SingleChoice {
            quorum: PercentageThreshold::Percent(Decimal::from_ratio(1u128, 10u128)),
        },
        Status::Rejected,
        None,
        None,
        true,
    );
}

#[test]
fn test_cant_vote_executed_or_closed() {
    // Create a proposal that will be rejected
    let (mut app, governance_addr) = do_test_votes_cw20_balances(
        vec![TestMultipleChoiceVote {
            voter: "blue".to_string(),
            position: MultipleChoiceVote { option_id: 2 },
            weight: Uint128::new(10),
            should_execute: ShouldExecute::Yes,
        }],
        VotingStrategy::SingleChoice {
            quorum: PercentageThreshold::Majority {},
        },
        Status::Rejected,
        None,
        None,
        true,
    );

    let gov_state: cw_core::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(governance_addr, &cw_core::msg::QueryMsg::DumpState {})
        .unwrap();
    let governance_modules = gov_state.proposal_modules;

    assert_eq!(governance_modules.len(), 1);
    let govmod = governance_modules.into_iter().next().unwrap().address;

    // Close the proposal
    app.execute_contract(
        Addr::unchecked("blue"),
        govmod.clone(),
        &ExecuteMsg::Close { proposal_id: 1 },
        &[],
    )
    .unwrap();

    // Try to vote, should error
    app.execute_contract(
        Addr::unchecked("blue"),
        govmod.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 1,
            vote: MultipleChoiceVote { option_id: 0 },
        },
        &[],
    )
    .unwrap_err();

    // Create a proposal that will pass
    let (mut app, _governance_addr) = do_test_votes_cw20_balances(
        vec![TestMultipleChoiceVote {
            voter: "blue".to_string(),
            position: MultipleChoiceVote { option_id: 0 },
            weight: Uint128::new(10),
            should_execute: ShouldExecute::Yes,
        }],
        VotingStrategy::SingleChoice {
            quorum: PercentageThreshold::Majority {},
        },
        Status::Passed,
        None,
        None,
        true,
    );

    // Execute the proposal
    app.execute_contract(
        Addr::unchecked("blue"),
        govmod.clone(),
        &ExecuteMsg::Execute { proposal_id: 1 },
        &[],
    )
    .unwrap();

    // Try to vote, should error
    app.execute_contract(
        Addr::unchecked("blue"),
        govmod,
        &ExecuteMsg::Vote {
            proposal_id: 1,
            vote: MultipleChoiceVote { option_id: 0 },
        },
        &[],
    )
    .unwrap_err();
}

#[test]
fn test_cant_propose_zero_power() {
    let mut app = App::default();
    let govmod_id = app.store_code(proposal_contract());
    let quorum = PercentageThreshold::Percent(Decimal::percent(10));
    let voting_strategy = VotingStrategy::SingleChoice { quorum };
    let max_voting_period = cw_utils::Duration::Height(6);
    let instantiate = InstantiateMsg {
        min_voting_period: None,
        close_proposal_on_execution_failure: true,
        max_voting_period,
        only_members_execute: false,
        allow_revoting: false,
        deposit_info: Some(DepositInfo {
            token: DepositToken::VotingModuleToken {},
            deposit: Uint128::new(1),
            refund_failed_proposals: true,
        }),
        voting_strategy,
    };

    let core_addr = instantiate_with_cw20_balances_governance(
        &mut app,
        govmod_id,
        to_binary(&instantiate).unwrap(),
        Some(vec![
            Cw20Coin {
                address: "blue".to_string(),
                amount: Uint128::new(1),
            },
            Cw20Coin {
                address: "blue2".to_string(),
                amount: Uint128::new(10),
            },
        ]),
    );

    let gov_state: cw_core::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(core_addr, &cw_core::msg::QueryMsg::DumpState {})
        .unwrap();
    let proposal_modules = gov_state.proposal_modules;

    assert_eq!(proposal_modules.len(), 1);
    let govmod = proposal_modules.into_iter().next().unwrap().address;

    let options = vec![
        MultipleChoiceOption {
            description: "multiple choice option 1".to_string(),
            msgs: None,
        },
        MultipleChoiceOption {
            description: "multiple choice option 2".to_string(),
            msgs: None,
        },
    ];

    let mc_options = MultipleChoiceOptions { options };

    let config: Config = app
        .wrap()
        .query_wasm_smart(govmod.clone(), &QueryMsg::Config {})
        .unwrap();
    if let Some(CheckedDepositInfo {
        ref token, deposit, ..
    }) = config.deposit_info
    {
        app.execute_contract(
            Addr::unchecked("blue"),
            token.clone(),
            &cw20_base::msg::ExecuteMsg::IncreaseAllowance {
                spender: govmod.to_string(),
                amount: deposit,
                expires: None,
            },
            &[],
        )
        .unwrap();
    }

    // Blue proposes
    app.execute_contract(
        Addr::unchecked("blue"),
        govmod.clone(),
        &ExecuteMsg::Propose {
            title: "A simple text proposal".to_string(),
            description: "A simple text proposal".to_string(),
            choices: mc_options.clone(),
        },
        &[],
    )
    .unwrap();

    // Should fail as blue's balance is now 0
    let err = app
        .execute_contract(
            Addr::unchecked("blue"),
            govmod,
            &ExecuteMsg::Propose {
                title: "A simple text proposal".to_string(),
                description: "A simple text proposal".to_string(),
                choices: mc_options,
            },
            &[],
        )
        .unwrap_err();

    assert!(matches!(
        err.downcast().unwrap(),
        ContractError::MustHaveVotingPower {}
    ))
}

#[test]
fn test_cant_vote_not_registered() {
    let (mut app, governance_addr) = do_test_votes_cw20_balances(
        vec![TestMultipleChoiceVote {
            voter: "blue".to_string(),
            position: MultipleChoiceVote { option_id: 2 },
            weight: Uint128::new(10),
            should_execute: ShouldExecute::Yes,
        }],
        VotingStrategy::SingleChoice {
            quorum: PercentageThreshold::Majority {},
        },
        Status::Open,
        Some(Uint128::new(100)),
        Some(DepositInfo {
            token: DepositToken::VotingModuleToken {},
            deposit: Uint128::new(1),
            refund_failed_proposals: true,
        }),
        false,
    );

    let gov_state: cw_core::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(governance_addr, &cw_core::msg::QueryMsg::DumpState {})
        .unwrap();
    let governance_modules = gov_state.proposal_modules;

    assert_eq!(governance_modules.len(), 1);
    let govmod = governance_modules.into_iter().next().unwrap().address;

    // Should error as blue2 is not registered to vote
    let err = app
        .execute_contract(
            Addr::unchecked("blue2"),
            govmod,
            &ExecuteMsg::Vote {
                proposal_id: 1,
                vote: MultipleChoiceVote { option_id: 0 },
            },
            &[],
        )
        .unwrap_err();

    assert!(matches!(
        err.downcast().unwrap(),
        ContractError::NotRegistered {}
    ))
}

#[test]
fn test_cant_execute_not_member() {
    // Create proposal with only_members_execute: true
    let mut app = App::default();
    let govmod_id = app.store_code(proposal_contract());

    let max_voting_period = cw_utils::Duration::Height(6);
    let quorum = PercentageThreshold::Majority {};

    let voting_strategy = VotingStrategy::SingleChoice { quorum };

    let instantiate = InstantiateMsg {
        min_voting_period: None,
        close_proposal_on_execution_failure: true,
        max_voting_period,
        only_members_execute: true,
        allow_revoting: false,
        deposit_info: None,
        voting_strategy,
    };

    let governance_addr = instantiate_with_cw20_balances_governance(
        &mut app,
        govmod_id,
        to_binary(&instantiate).unwrap(),
        Some(vec![Cw20Coin {
            address: "blue".to_string(),
            amount: Uint128::new(10),
        }]),
    );

    let governance_modules: Vec<ProposalModule> = app
        .wrap()
        .query_wasm_smart(
            governance_addr.clone(),
            &cw_core::msg::QueryMsg::ProposalModules {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(governance_modules.len(), 1);

    let gov_state: cw_core::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(governance_addr, &cw_core::msg::QueryMsg::DumpState {})
        .unwrap();
    let governance_modules = gov_state.proposal_modules;

    assert_eq!(governance_modules.len(), 1);
    let govmod = governance_modules.into_iter().next().unwrap().address;

    // Create proposal
    let options = vec![
        MultipleChoiceOption {
            description: "multiple choice option 1".to_string(),
            msgs: None,
        },
        MultipleChoiceOption {
            description: "multiple choice option 2".to_string(),
            msgs: None,
        },
    ];

    let mc_options = MultipleChoiceOptions { options };

    app.execute_contract(
        Addr::unchecked("blue"),
        govmod.clone(),
        &ExecuteMsg::Propose {
            title: "A simple text proposal".to_string(),
            description: "A simple text proposal".to_string(),
            choices: mc_options,
        },
        &[],
    )
    .unwrap();

    // Proposal should pass after this vote
    app.execute_contract(
        Addr::unchecked("blue"),
        govmod.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 1,
            vote: MultipleChoiceVote { option_id: 0 },
        },
        &[],
    )
    .unwrap();

    // Execute should error as blue2 is not a member
    let err = app
        .execute_contract(
            Addr::unchecked("blue2"),
            govmod,
            &ExecuteMsg::Execute { proposal_id: 1 },
            &[],
        )
        .unwrap_err();

    assert!(matches!(
        err.downcast().unwrap(),
        ContractError::Unauthorized {}
    ))
}

#[test]
fn test_close_open_proposal() {
    let (mut app, governance_addr) = do_test_votes_cw20_balances(
        vec![TestMultipleChoiceVote {
            voter: "blue".to_string(),
            position: MultipleChoiceVote { option_id: 2 },
            weight: Uint128::new(10),
            should_execute: ShouldExecute::Yes,
        }],
        VotingStrategy::SingleChoice {
            quorum: PercentageThreshold::Majority {},
        },
        Status::Open,
        Some(Uint128::new(100)),
        Some(DepositInfo {
            token: DepositToken::VotingModuleToken {},
            deposit: Uint128::new(1),
            refund_failed_proposals: true,
        }),
        false,
    );

    let gov_state: cw_core::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(governance_addr, &cw_core::msg::QueryMsg::DumpState {})
        .unwrap();
    let governance_modules = gov_state.proposal_modules;

    assert_eq!(governance_modules.len(), 1);
    let govmod = governance_modules.into_iter().next().unwrap().address;

    // Close the proposal, this should error as the proposal is still
    // open and not expired.
    app.execute_contract(
        Addr::unchecked("blue"),
        govmod.clone(),
        &ExecuteMsg::Close { proposal_id: 1 },
        &[],
    )
    .unwrap_err();

    // Make the proposal expire.
    app.update_block(|block| block.height += 10);

    // Close the proposal, this should work as the proposal is now
    // open and expired.
    app.execute_contract(
        Addr::unchecked("blue"),
        govmod.clone(),
        &ExecuteMsg::Close { proposal_id: 1 },
        &[],
    )
    .unwrap();

    // Check that a refund was issued.
    let govmod_config: Config = app
        .wrap()
        .query_wasm_smart(govmod, &QueryMsg::Config {})
        .unwrap();
    let CheckedDepositInfo { token, .. } = govmod_config.deposit_info.unwrap();
    let balance: cw20::BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            token,
            &cw20::Cw20QueryMsg::Balance {
                address: "blue".to_string(),
            },
        )
        .unwrap();

    // Proposal been closed so deposit has been
    // refunded.
    assert_eq!(balance.balance, Uint128::new(10));
}

#[test]
fn test_no_refund_failed_proposal() {
    let (mut app, governance_addr) = do_test_votes_cw20_balances(
        vec![TestMultipleChoiceVote {
            voter: "blue".to_string(),
            position: MultipleChoiceVote { option_id: 2 },
            weight: Uint128::new(10),
            should_execute: ShouldExecute::Yes,
        }],
        VotingStrategy::SingleChoice {
            quorum: PercentageThreshold::Majority {},
        },
        Status::Open,
        Some(Uint128::new(100)),
        Some(DepositInfo {
            token: DepositToken::VotingModuleToken {},
            deposit: Uint128::new(1),
            refund_failed_proposals: false,
        }),
        false,
    );

    let gov_state: cw_core::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(governance_addr, &cw_core::msg::QueryMsg::DumpState {})
        .unwrap();
    let governance_modules = gov_state.proposal_modules;

    assert_eq!(governance_modules.len(), 1);
    let govmod = governance_modules.into_iter().next().unwrap().address;

    // Make the proposal expire.
    app.update_block(|block| block.height += 10);

    // Close the proposal, this should work as the proposal is now
    // open and expired.
    app.execute_contract(
        Addr::unchecked("blue"),
        govmod.clone(),
        &ExecuteMsg::Close { proposal_id: 1 },
        &[],
    )
    .unwrap();

    // Check that a refund was issued.
    let govmod_config: Config = app
        .wrap()
        .query_wasm_smart(govmod, &QueryMsg::Config {})
        .unwrap();
    let CheckedDepositInfo { token, .. } = govmod_config.deposit_info.unwrap();
    let balance: cw20::BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            token,
            &cw20::Cw20QueryMsg::Balance {
                address: "blue".to_string(),
            },
        )
        .unwrap();

    // No refund should have been issued.
    assert_eq!(balance.balance, Uint128::new(9));
}

#[test]
fn test_zero_deposit() {
    do_test_votes_cw20_balances(
        vec![TestMultipleChoiceVote {
            voter: "blue".to_string(),
            position: MultipleChoiceVote { option_id: 0 },
            weight: Uint128::new(10),
            should_execute: ShouldExecute::Yes,
        }],
        VotingStrategy::SingleChoice {
            quorum: PercentageThreshold::Majority {},
        },
        Status::Passed,
        None,
        Some(DepositInfo {
            token: DepositToken::VotingModuleToken {},
            deposit: Uint128::new(0),
            refund_failed_proposals: false,
        }),
        true,
    );
}

#[test]
fn test_deposit_return_on_close() {
    let quorum = PercentageThreshold::Percent(Decimal::percent(10));
    let voting_strategy = VotingStrategy::SingleChoice { quorum };

    let (mut app, governance_addr) = do_test_votes_cw20_balances(
        vec![TestMultipleChoiceVote {
            voter: "blue".to_string(),
            position: MultipleChoiceVote { option_id: 2 },
            weight: Uint128::new(10),
            should_execute: ShouldExecute::Yes,
        }],
        voting_strategy,
        Status::Rejected,
        None,
        Some(DepositInfo {
            token: DepositToken::VotingModuleToken {},
            deposit: Uint128::new(1),
            refund_failed_proposals: true,
        }),
        false,
    );
    let gov_state: cw_core::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(governance_addr, &cw_core::msg::QueryMsg::DumpState {})
        .unwrap();
    let governance_modules = gov_state.proposal_modules;

    assert_eq!(governance_modules.len(), 1);
    let govmod = governance_modules.into_iter().next().unwrap().address;

    let govmod_config: Config = app
        .wrap()
        .query_wasm_smart(govmod.clone(), &QueryMsg::Config {})
        .unwrap();
    let CheckedDepositInfo { token, .. } = govmod_config.deposit_info.unwrap();
    let balance: cw20::BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            token.clone(),
            &cw20::Cw20QueryMsg::Balance {
                address: "blue".to_string(),
            },
        )
        .unwrap();

    // Proposal has not been closed so deposit has not been
    // refunded.
    assert_eq!(balance.balance, Uint128::new(9));

    // Close the proposal, this should cause the deposit to be
    // refunded.
    app.execute_contract(
        Addr::unchecked("blue"),
        govmod,
        &ExecuteMsg::Close { proposal_id: 1 },
        &[],
    )
    .unwrap();

    let balance: cw20::BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            token,
            &cw20::Cw20QueryMsg::Balance {
                address: "blue".to_string(),
            },
        )
        .unwrap();

    // Proposal has been closed so deposit has been refunded.
    assert_eq!(balance.balance, Uint128::new(10));
}

#[test]
fn test_execute_expired_proposal() {
    let mut app = App::default();
    let govmod_id = app.store_code(proposal_contract());
    let quorum = PercentageThreshold::Percent(Decimal::percent(10));
    let voting_strategy = VotingStrategy::SingleChoice { quorum };
    let max_voting_period = cw_utils::Duration::Height(6);
    let instantiate = InstantiateMsg {
        min_voting_period: None,
        close_proposal_on_execution_failure: true,
        max_voting_period,
        only_members_execute: false,
        allow_revoting: false,
        deposit_info: None,
        voting_strategy,
    };

    let core_addr = instantiate_with_staked_balances_governance(
        &mut app,
        govmod_id,
        to_binary(&instantiate).unwrap(),
        Some(vec![
            Cw20Coin {
                address: "blue".to_string(),
                amount: Uint128::new(10),
            },
            Cw20Coin {
                address: "inactive".to_string(),
                amount: Uint128::new(90),
            },
        ]),
    );

    let gov_state: cw_core::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(core_addr, &cw_core::msg::QueryMsg::DumpState {})
        .unwrap();
    let proposal_modules = gov_state.proposal_modules;

    assert_eq!(proposal_modules.len(), 1);
    let govmod = proposal_modules.into_iter().next().unwrap().address;

    let options = vec![
        MultipleChoiceOption {
            description: "multiple choice option 1".to_string(),
            msgs: None,
        },
        MultipleChoiceOption {
            description: "multiple choice option 2".to_string(),
            msgs: None,
        },
    ];

    let mc_options = MultipleChoiceOptions { options };

    app.execute_contract(
        Addr::unchecked("blue"),
        govmod.clone(),
        &ExecuteMsg::Propose {
            title: "A simple text proposal".to_string(),
            description: "A simple text proposal".to_string(),
            choices: mc_options,
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        Addr::unchecked("blue"),
        govmod.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 1,
            vote: MultipleChoiceVote { option_id: 0 },
        },
        &[],
    )
    .unwrap();

    // Proposal has now reached quorum but should not be passed.
    let proposal: ProposalResponse = app
        .wrap()
        .query_wasm_smart(govmod.clone(), &QueryMsg::Proposal { proposal_id: 1 })
        .unwrap();
    assert_eq!(proposal.proposal.status, Status::Open);

    // Expire the proposal. It should now be passed as quorum was reached.
    app.update_block(|b| b.height += 10);

    let proposal: ProposalResponse = app
        .wrap()
        .query_wasm_smart(govmod.clone(), &QueryMsg::Proposal { proposal_id: 1 })
        .unwrap();
    assert_eq!(proposal.proposal.status, Status::Passed);

    // Try to close the proposal. This should fail as the proposal is
    // passed.
    app.execute_contract(
        Addr::unchecked("blue"),
        govmod.clone(),
        &ExecuteMsg::Close { proposal_id: 1 },
        &[],
    )
    .unwrap_err();

    // Check that we can execute the proposal despite the fact that it
    // is technically expired.
    app.execute_contract(
        Addr::unchecked("blue"),
        govmod.clone(),
        &ExecuteMsg::Execute { proposal_id: 1 },
        &[],
    )
    .unwrap();

    // Can't execute more than once.
    app.execute_contract(
        Addr::unchecked("blue"),
        govmod.clone(),
        &ExecuteMsg::Execute { proposal_id: 1 },
        &[],
    )
    .unwrap_err();

    let proposal: ProposalResponse = app
        .wrap()
        .query_wasm_smart(govmod, &QueryMsg::Proposal { proposal_id: 1 })
        .unwrap();
    assert_eq!(proposal.proposal.status, Status::Executed);
}

#[test]
fn test_update_config() {
    let (mut app, governance_addr) = do_test_votes_cw20_balances(
        vec![TestMultipleChoiceVote {
            voter: "blue".to_string(),
            position: MultipleChoiceVote { option_id: 0 },
            weight: Uint128::new(10),
            should_execute: ShouldExecute::Yes,
        }],
        VotingStrategy::SingleChoice {
            quorum: PercentageThreshold::Majority {},
        },
        Status::Passed,
        None,
        Some(DepositInfo {
            token: DepositToken::VotingModuleToken {},
            deposit: Uint128::new(1),
            refund_failed_proposals: false,
        }),
        false,
    );

    let gov_state: cw_core::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(governance_addr, &cw_core::msg::QueryMsg::DumpState {})
        .unwrap();
    let governance_modules = gov_state.proposal_modules;

    assert_eq!(governance_modules.len(), 1);
    let govmod = governance_modules.into_iter().next().unwrap().address;

    let govmod_config: Config = app
        .wrap()
        .query_wasm_smart(govmod.clone(), &QueryMsg::Config {})
        .unwrap();

    assert_eq!(
        govmod_config.voting_strategy,
        VotingStrategy::SingleChoice {
            quorum: PercentageThreshold::Majority {}
        }
    );

    let dao = govmod_config.dao;

    // Attempt to update the config from a non-dao address. This
    // should fail as it is unauthorized.
    app.execute_contract(
        Addr::unchecked("wrong"),
        govmod.clone(),
        &ExecuteMsg::UpdateConfig {
            voting_strategy: VotingStrategy::SingleChoice {
                quorum: PercentageThreshold::Majority {},
            },
            min_voting_period: None,
            close_proposal_on_execution_failure: true,
            max_voting_period: cw_utils::Duration::Height(10),
            only_members_execute: false,
            allow_revoting: false,
            dao: dao.to_string(),
            deposit_info: None,
        },
        &[],
    )
    .unwrap_err();

    // Update the config from the DAO address. This should succeed.
    app.execute_contract(
        dao.clone(),
        govmod.clone(),
        &ExecuteMsg::UpdateConfig {
            voting_strategy: VotingStrategy::SingleChoice {
                quorum: PercentageThreshold::Majority {},
            },
            min_voting_period: None,
            close_proposal_on_execution_failure: true,
            max_voting_period: cw_utils::Duration::Height(10),
            only_members_execute: false,
            allow_revoting: false,
            dao: Addr::unchecked(CREATOR_ADDR).to_string(),
            deposit_info: None,
        },
        &[],
    )
    .unwrap();

    let govmod_config: Config = app
        .wrap()
        .query_wasm_smart(govmod.clone(), &QueryMsg::Config {})
        .unwrap();

    let expected = Config {
        voting_strategy: VotingStrategy::SingleChoice {
            quorum: PercentageThreshold::Majority {},
        },
        min_voting_period: None,
        close_proposal_on_execution_failure: true,
        max_voting_period: cw_utils::Duration::Height(10),
        only_members_execute: false,
        allow_revoting: false,
        dao: Addr::unchecked(CREATOR_ADDR),
        deposit_info: None,
    };
    assert_eq!(govmod_config, expected);

    // As we have changed the DAO address updating the config using
    // the original one should now fail.
    app.execute_contract(
        dao,
        govmod,
        &ExecuteMsg::UpdateConfig {
            voting_strategy: VotingStrategy::SingleChoice {
                quorum: PercentageThreshold::Majority {},
            },
            min_voting_period: None,
            close_proposal_on_execution_failure: true,
            max_voting_period: cw_utils::Duration::Height(10),
            only_members_execute: false,
            allow_revoting: false,
            dao: Addr::unchecked(CREATOR_ADDR).to_string(),
            deposit_info: None,
        },
        &[],
    )
    .unwrap_err();
}

#[test]
fn test_no_return_if_no_refunds() {
    let (mut app, governance_addr) = do_test_votes_cw20_balances(
        vec![TestMultipleChoiceVote {
            voter: "blue".to_string(),
            position: MultipleChoiceVote { option_id: 2 },
            weight: Uint128::new(10),
            should_execute: ShouldExecute::Yes,
        }],
        VotingStrategy::SingleChoice {
            quorum: PercentageThreshold::Majority {},
        },
        Status::Rejected,
        None,
        Some(DepositInfo {
            token: DepositToken::VotingModuleToken {},
            deposit: Uint128::new(1),
            refund_failed_proposals: false,
        }),
        true,
    );
    let gov_state: cw_core::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(governance_addr, &cw_core::msg::QueryMsg::DumpState {})
        .unwrap();
    let governance_modules = gov_state.proposal_modules;

    assert_eq!(governance_modules.len(), 1);
    let govmod = governance_modules.into_iter().next().unwrap().address;

    let govmod_config: Config = app
        .wrap()
        .query_wasm_smart(govmod.clone(), &QueryMsg::Config {})
        .unwrap();
    let CheckedDepositInfo { token, .. } = govmod_config.deposit_info.unwrap();

    // Close the proposal, this should cause the deposit to be
    // refunded.
    app.execute_contract(
        Addr::unchecked("blue"),
        govmod,
        &ExecuteMsg::Close { proposal_id: 1 },
        &[],
    )
    .unwrap();

    let balance: cw20::BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            token,
            &cw20::Cw20QueryMsg::Balance {
                address: "blue".to_string(),
            },
        )
        .unwrap();

    // Proposal has been closed but deposit has not been refunded.
    assert_eq!(balance.balance, Uint128::new(9));
}

#[test]
fn test_query_list_proposals() {
    let mut app = App::default();
    let govmod_id = app.store_code(proposal_contract());
    let quorum = PercentageThreshold::Majority {};
    let voting_strategy = VotingStrategy::SingleChoice { quorum };
    let max_voting_period = cw_utils::Duration::Height(6);
    let instantiate = InstantiateMsg {
        min_voting_period: None,
        close_proposal_on_execution_failure: true,
        max_voting_period,
        only_members_execute: false,
        allow_revoting: false,
        deposit_info: None,
        voting_strategy: voting_strategy.clone(),
    };
    let gov_addr = instantiate_with_cw20_balances_governance(
        &mut app,
        govmod_id,
        to_binary(&instantiate).unwrap(),
        Some(vec![Cw20Coin {
            address: CREATOR_ADDR.to_string(),
            amount: Uint128::new(100),
        }]),
    );

    let gov_modules: Vec<ProposalModule> = app
        .wrap()
        .query_wasm_smart(
            gov_addr,
            &cw_core::msg::QueryMsg::ProposalModules {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(gov_modules.len(), 1);

    let govmod = gov_modules.into_iter().next().unwrap().address;

    let options = vec![
        MultipleChoiceOption {
            description: "multiple choice option 1".to_string(),
            msgs: None,
        },
        MultipleChoiceOption {
            description: "multiple choice option 2".to_string(),
            msgs: None,
        },
    ];

    let mc_options = MultipleChoiceOptions { options };

    for _i in 1..10 {
        app.execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            govmod.clone(),
            &ExecuteMsg::Propose {
                title: "A simple text proposal".to_string(),
                description: "A simple text proposal".to_string(),
                choices: mc_options.clone(),
            },
            &[],
        )
        .unwrap();
    }

    let proposals_forward: ProposalListResponse = app
        .wrap()
        .query_wasm_smart(
            govmod.clone(),
            &QueryMsg::ListProposals {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    let mut proposals_backward: ProposalListResponse = app
        .wrap()
        .query_wasm_smart(
            govmod.clone(),
            &QueryMsg::ReverseProposals {
                start_before: None,
                limit: None,
            },
        )
        .unwrap();

    proposals_backward.proposals.reverse();

    assert_eq!(proposals_forward.proposals, proposals_backward.proposals);
    let checked_options = mc_options.into_checked().unwrap();
    let current_block = app.block_info();
    let expected = ProposalResponse {
        id: 1,
        proposal: MultipleChoiceProposal {
            title: "A simple text proposal".to_string(),
            description: "A simple text proposal".to_string(),
            proposer: Addr::unchecked(CREATOR_ADDR),
            start_height: current_block.height,
            expiration: max_voting_period.after(&current_block),
            choices: checked_options.options.clone(),
            status: Status::Open,
            voting_strategy: voting_strategy.clone(),
            total_power: Uint128::new(100),
            votes: MultipleChoiceVotes {
                vote_weights: vec![Uint128::zero(); 3],
            },
            allow_revoting: false,
            deposit_info: None,
            min_voting_period: None,
            created: current_block.time,
            last_updated: current_block.time,
        },
    };
    assert_eq!(proposals_forward.proposals[0], expected);

    // Get proposals (3, 5]
    let proposals_forward: ProposalListResponse = app
        .wrap()
        .query_wasm_smart(
            govmod.clone(),
            &QueryMsg::ListProposals {
                start_after: Some(3),
                limit: Some(2),
            },
        )
        .unwrap();
    let mut proposals_backward: ProposalListResponse = app
        .wrap()
        .query_wasm_smart(
            govmod,
            &QueryMsg::ReverseProposals {
                start_before: Some(6),
                limit: Some(2),
            },
        )
        .unwrap();

    let expected = ProposalResponse {
        id: 4,
        proposal: MultipleChoiceProposal {
            title: "A simple text proposal".to_string(),
            description: "A simple text proposal".to_string(),
            proposer: Addr::unchecked(CREATOR_ADDR),
            start_height: current_block.height,
            expiration: max_voting_period.after(&current_block),
            choices: checked_options.options,
            status: Status::Open,
            voting_strategy,
            total_power: Uint128::new(100),
            votes: MultipleChoiceVotes {
                vote_weights: vec![Uint128::zero(); 3],
            },
            allow_revoting: false,
            deposit_info: None,
            min_voting_period: None,
            created: current_block.time,
            last_updated: current_block.time,
        },
    };
    assert_eq!(proposals_forward.proposals[0], expected);
    assert_eq!(proposals_backward.proposals[1], expected);

    proposals_backward.proposals.reverse();
    assert_eq!(proposals_forward.proposals, proposals_backward.proposals);
}

#[test]
fn test_hooks() {
    let mut app = App::default();
    let govmod_id = app.store_code(proposal_contract());

    let quorum = PercentageThreshold::Majority {};
    let voting_strategy = VotingStrategy::SingleChoice { quorum };
    let max_voting_period = cw_utils::Duration::Height(6);
    let instantiate = InstantiateMsg {
        min_voting_period: None,
        close_proposal_on_execution_failure: true,
        max_voting_period,
        only_members_execute: false,
        allow_revoting: false,
        deposit_info: None,
        voting_strategy,
    };

    let governance_addr = instantiate_with_cw20_balances_governance(
        &mut app,
        govmod_id,
        to_binary(&instantiate).unwrap(),
        None,
    );
    let governance_modules: Vec<ProposalModule> = app
        .wrap()
        .query_wasm_smart(
            governance_addr,
            &cw_core::msg::QueryMsg::ProposalModules {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(governance_modules.len(), 1);
    let govmod = governance_modules.into_iter().next().unwrap().address;

    let govmod_config: Config = app
        .wrap()
        .query_wasm_smart(govmod.clone(), &QueryMsg::Config {})
        .unwrap();
    let dao = govmod_config.dao;

    // Expect no hooks
    let hooks: HooksResponse = app
        .wrap()
        .query_wasm_smart(govmod.clone(), &QueryMsg::ProposalHooks {})
        .unwrap();
    assert_eq!(hooks.hooks.len(), 0);

    let hooks: HooksResponse = app
        .wrap()
        .query_wasm_smart(govmod.clone(), &QueryMsg::VoteHooks {})
        .unwrap();
    assert_eq!(hooks.hooks.len(), 0);

    let msg = ExecuteMsg::AddProposalHook {
        address: "some_addr".to_string(),
    };

    // Expect error as sender is not DAO
    let _err = app
        .execute_contract(Addr::unchecked(CREATOR_ADDR), govmod.clone(), &msg, &[])
        .unwrap_err();

    // Expect success as sender is now DAO
    let _res = app
        .execute_contract(dao.clone(), govmod.clone(), &msg, &[])
        .unwrap();

    let hooks: HooksResponse = app
        .wrap()
        .query_wasm_smart(govmod.clone(), &QueryMsg::ProposalHooks {})
        .unwrap();
    assert_eq!(hooks.hooks.len(), 1);

    // Expect error as hook is already set
    let _err = app
        .execute_contract(dao.clone(), govmod.clone(), &msg, &[])
        .unwrap_err();

    // Expect error as hook does not exist
    let _err = app
        .execute_contract(
            dao.clone(),
            govmod.clone(),
            &ExecuteMsg::RemoveProposalHook {
                address: "not_exist".to_string(),
            },
            &[],
        )
        .unwrap_err();

    let msg = ExecuteMsg::RemoveProposalHook {
        address: "some_addr".to_string(),
    };

    // Expect error as sender is not DAO
    let _err = app
        .execute_contract(Addr::unchecked(CREATOR_ADDR), govmod.clone(), &msg, &[])
        .unwrap_err();

    // Expect success
    let _res = app
        .execute_contract(dao.clone(), govmod.clone(), &msg, &[])
        .unwrap();

    let msg = ExecuteMsg::AddVoteHook {
        address: "some_addr".to_string(),
    };

    // Expect error as sender is not DAO
    let _err = app
        .execute_contract(Addr::unchecked(CREATOR_ADDR), govmod.clone(), &msg, &[])
        .unwrap_err();

    // Expect success as sender is now DAO
    let _res = app
        .execute_contract(dao.clone(), govmod.clone(), &msg, &[])
        .unwrap();

    let hooks: HooksResponse = app
        .wrap()
        .query_wasm_smart(govmod.clone(), &QueryMsg::VoteHooks {})
        .unwrap();
    assert_eq!(hooks.hooks.len(), 1);

    // Expect error as hook is already set
    let _err = app
        .execute_contract(dao.clone(), govmod.clone(), &msg, &[])
        .unwrap_err();

    // Expect error as hook does not exist
    let _err = app
        .execute_contract(
            dao.clone(),
            govmod.clone(),
            &ExecuteMsg::RemoveVoteHook {
                address: "not_exist".to_string(),
            },
            &[],
        )
        .unwrap_err();

    let msg = ExecuteMsg::RemoveVoteHook {
        address: "some_addr".to_string(),
    };

    // Expect error as sender is not DAO
    let _err = app
        .execute_contract(Addr::unchecked(CREATOR_ADDR), govmod.clone(), &msg, &[])
        .unwrap_err();

    // Expect success
    let _res = app.execute_contract(dao, govmod, &msg, &[]).unwrap();
}

#[test]
fn test_active_threshold_absolute() {
    let mut app = App::default();
    let govmod_id = app.store_code(proposal_contract());

    let quorum = PercentageThreshold::Majority {};
    let voting_strategy = VotingStrategy::SingleChoice { quorum };
    let max_voting_period = cw_utils::Duration::Height(6);
    let instantiate = InstantiateMsg {
        min_voting_period: None,
        close_proposal_on_execution_failure: true,
        max_voting_period,
        only_members_execute: false,
        allow_revoting: false,
        deposit_info: None,
        voting_strategy,
    };

    let governance_addr = instantiate_with_staking_active_threshold(
        &mut app,
        govmod_id,
        to_binary(&instantiate).unwrap(),
        None,
        Some(ActiveThreshold::AbsoluteCount {
            count: Uint128::new(100),
        }),
    );
    let governance_modules: Vec<ProposalModule> = app
        .wrap()
        .query_wasm_smart(
            governance_addr,
            &cw_core::msg::QueryMsg::ProposalModules {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(governance_modules.len(), 1);
    let govmod = governance_modules.into_iter().next().unwrap().address;

    let govmod_config: Config = app
        .wrap()
        .query_wasm_smart(govmod.clone(), &QueryMsg::Config {})
        .unwrap();
    let dao = govmod_config.dao;
    let voting_module: Addr = app
        .wrap()
        .query_wasm_smart(dao, &cw_core::msg::QueryMsg::VotingModule {})
        .unwrap();
    let staking_contract: Addr = app
        .wrap()
        .query_wasm_smart(
            voting_module.clone(),
            &cw20_staked_balance_voting::msg::QueryMsg::StakingContract {},
        )
        .unwrap();
    let token_contract: Addr = app
        .wrap()
        .query_wasm_smart(
            voting_module,
            &cw_core_interface::voting::Query::TokenContract {},
        )
        .unwrap();

    let options = vec![
        MultipleChoiceOption {
            description: "multiple choice option 1".to_string(),
            msgs: None,
        },
        MultipleChoiceOption {
            description: "multiple choice option 2".to_string(),
            msgs: None,
        },
    ];

    let mc_options = MultipleChoiceOptions { options };

    // Try and create a proposal, will fail as inactive
    let _err = app
        .execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            govmod.clone(),
            &crate::msg::ExecuteMsg::Propose {
                title: "A simple text proposal".to_string(),
                description: "This is a simple text proposal".to_string(),
                choices: mc_options.clone(),
            },
            &[],
        )
        .unwrap_err();

    // Stake enough tokens
    let msg = cw20::Cw20ExecuteMsg::Send {
        contract: staking_contract.to_string(),
        amount: Uint128::new(100),
        msg: to_binary(&cw20_stake::msg::ReceiveMsg::Stake {}).unwrap(),
    };
    app.execute_contract(Addr::unchecked(CREATOR_ADDR), token_contract, &msg, &[])
        .unwrap();
    app.update_block(next_block);

    // Try and create a proposal, will now succeed as enough tokens are staked
    let _res = app
        .execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            govmod.clone(),
            &crate::msg::ExecuteMsg::Propose {
                title: "A simple text proposal".to_string(),
                description: "This is a simple text proposal".to_string(),
                choices: mc_options.clone(),
            },
            &[],
        )
        .unwrap();

    // Unstake some tokens to make it inactive again
    let msg = cw20_stake::msg::ExecuteMsg::Unstake {
        amount: Uint128::new(50),
    };
    app.execute_contract(Addr::unchecked(CREATOR_ADDR), staking_contract, &msg, &[])
        .unwrap();
    app.update_block(next_block);

    // Try and create a proposal, will fail as no longer active
    let _err = app
        .execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            govmod,
            &crate::msg::ExecuteMsg::Propose {
                title: "A simple text proposal".to_string(),
                description: "This is a simple text proposal".to_string(),
                choices: mc_options,
            },
            &[],
        )
        .unwrap_err();
}

#[test]
fn test_active_threshold_percent() {
    let mut app = App::default();
    let govmod_id = app.store_code(proposal_contract());
    let quorum = PercentageThreshold::Majority {};
    let voting_strategy = VotingStrategy::SingleChoice { quorum };
    let max_voting_period = cw_utils::Duration::Height(6);
    let instantiate = InstantiateMsg {
        min_voting_period: None,
        close_proposal_on_execution_failure: true,
        max_voting_period,
        only_members_execute: false,
        allow_revoting: false,
        deposit_info: None,
        voting_strategy,
    };

    // 20% needed to be active, 20% of 100000000 is 20000000
    let governance_addr = instantiate_with_staking_active_threshold(
        &mut app,
        govmod_id,
        to_binary(&instantiate).unwrap(),
        None,
        Some(ActiveThreshold::Percentage {
            percent: Decimal::percent(20),
        }),
    );
    let governance_modules: Vec<ProposalModule> = app
        .wrap()
        .query_wasm_smart(
            governance_addr,
            &cw_core::msg::QueryMsg::ProposalModules {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(governance_modules.len(), 1);
    let govmod = governance_modules.into_iter().next().unwrap().address;

    let govmod_config: Config = app
        .wrap()
        .query_wasm_smart(govmod.clone(), &QueryMsg::Config {})
        .unwrap();
    let dao = govmod_config.dao;
    let voting_module: Addr = app
        .wrap()
        .query_wasm_smart(dao, &cw_core::msg::QueryMsg::VotingModule {})
        .unwrap();
    let staking_contract: Addr = app
        .wrap()
        .query_wasm_smart(
            voting_module.clone(),
            &cw20_staked_balance_voting::msg::QueryMsg::StakingContract {},
        )
        .unwrap();
    let token_contract: Addr = app
        .wrap()
        .query_wasm_smart(
            voting_module,
            &cw_core_interface::voting::Query::TokenContract {},
        )
        .unwrap();

    let options = vec![
        MultipleChoiceOption {
            description: "multiple choice option 1".to_string(),
            msgs: None,
        },
        MultipleChoiceOption {
            description: "multiple choice option 2".to_string(),
            msgs: None,
        },
    ];

    let mc_options = MultipleChoiceOptions { options };

    // Try and create a proposal, will fail as inactive
    let _res = app
        .execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            govmod.clone(),
            &ExecuteMsg::Propose {
                title: "A simple text proposal".to_string(),
                description: "A simple text proposal".to_string(),
                choices: mc_options.clone(),
            },
            &[],
        )
        .unwrap_err();

    // Stake enough tokens
    let msg = cw20::Cw20ExecuteMsg::Send {
        contract: staking_contract.to_string(),
        amount: Uint128::new(20000000),
        msg: to_binary(&cw20_stake::msg::ReceiveMsg::Stake {}).unwrap(),
    };
    app.execute_contract(Addr::unchecked(CREATOR_ADDR), token_contract, &msg, &[])
        .unwrap();
    app.update_block(next_block);

    // Try and create a proposal, will now succeed as enough tokens are staked
    let _res = app
        .execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            govmod.clone(),
            &ExecuteMsg::Propose {
                title: "A simple text proposal".to_string(),
                description: "A simple text proposal".to_string(),
                choices: mc_options.clone(),
            },
            &[],
        )
        .unwrap();

    // Unstake some tokens to make it inactive again
    let msg = cw20_stake::msg::ExecuteMsg::Unstake {
        amount: Uint128::new(1000),
    };
    app.execute_contract(Addr::unchecked(CREATOR_ADDR), staking_contract, &msg, &[])
        .unwrap();
    app.update_block(next_block);

    // Try and create a proposal, will fail as no longer active
    let _res = app
        .execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            govmod,
            &ExecuteMsg::Propose {
                title: "A simple text proposal".to_string(),
                description: "A simple text proposal".to_string(),
                choices: mc_options,
            },
            &[],
        )
        .unwrap_err();
}

#[test]
fn test_active_threshold_none() {
    let mut app = App::default();
    let govmod_id = app.store_code(proposal_contract());
    let quorum = PercentageThreshold::Majority {};
    let voting_strategy = VotingStrategy::SingleChoice { quorum };
    let max_voting_period = cw_utils::Duration::Height(6);
    let instantiate = InstantiateMsg {
        min_voting_period: None,
        close_proposal_on_execution_failure: true,
        max_voting_period,
        only_members_execute: false,
        allow_revoting: false,
        deposit_info: None,
        voting_strategy,
    };

    let governance_addr = instantiate_with_staking_active_threshold(
        &mut app,
        govmod_id,
        to_binary(&instantiate).unwrap(),
        None,
        None,
    );
    let governance_modules: Vec<ProposalModule> = app
        .wrap()
        .query_wasm_smart(
            governance_addr,
            &cw_core::msg::QueryMsg::ProposalModules {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(governance_modules.len(), 1);
    let govmod = governance_modules.into_iter().next().unwrap().address;

    let govmod_config: Config = app
        .wrap()
        .query_wasm_smart(govmod.clone(), &QueryMsg::Config {})
        .unwrap();
    let dao = govmod_config.dao;
    let voting_module: Addr = app
        .wrap()
        .query_wasm_smart(dao, &cw_core::msg::QueryMsg::VotingModule {})
        .unwrap();
    let staking_contract: Addr = app
        .wrap()
        .query_wasm_smart(
            voting_module.clone(),
            &cw20_staked_balance_voting::msg::QueryMsg::StakingContract {},
        )
        .unwrap();
    let token_contract: Addr = app
        .wrap()
        .query_wasm_smart(
            voting_module,
            &cw_core_interface::voting::Query::TokenContract {},
        )
        .unwrap();

    // Stake some tokens so we can propose
    let msg = cw20::Cw20ExecuteMsg::Send {
        contract: staking_contract.to_string(),
        amount: Uint128::new(2000),
        msg: to_binary(&cw20_stake::msg::ReceiveMsg::Stake {}).unwrap(),
    };
    app.execute_contract(Addr::unchecked(CREATOR_ADDR), token_contract, &msg, &[])
        .unwrap();
    app.update_block(next_block);

    let options = vec![
        MultipleChoiceOption {
            description: "multiple choice option 1".to_string(),
            msgs: None,
        },
        MultipleChoiceOption {
            description: "multiple choice option 2".to_string(),
            msgs: None,
        },
    ];

    let mc_options = MultipleChoiceOptions { options };

    // Try and create a proposal, will succeed as no threshold
    let _res = app
        .execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            govmod,
            &ExecuteMsg::Propose {
                title: "A simple text proposal".to_string(),
                description: "A simple text proposal".to_string(),
                choices: mc_options.clone(),
            },
            &[],
        )
        .unwrap();

    // Now try with balance voting to test when IsActive is not implemented
    // on the contract
    let _threshold = Threshold::AbsolutePercentage {
        percentage: PercentageThreshold::Majority {},
    };
    let _max_voting_period = cw_utils::Duration::Height(6);

    let governance_addr = instantiate_with_cw20_balances_governance(
        &mut app,
        govmod_id,
        to_binary(&instantiate).unwrap(),
        None,
    );
    let governance_modules: Vec<ProposalModule> = app
        .wrap()
        .query_wasm_smart(
            governance_addr,
            &cw_core::msg::QueryMsg::ProposalModules {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(governance_modules.len(), 1);
    let govmod = governance_modules.into_iter().next().unwrap().address;

    // Try and create a proposal, will succeed as IsActive is not implemented
    let _res = app
        .execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            govmod,
            &ExecuteMsg::Propose {
                title: "A simple text proposal".to_string(),
                description: "A simple text proposal".to_string(),
                choices: mc_options,
            },
            &[],
        )
        .unwrap();
}

/// Basic test for revoting on prop-multiple
#[test]
fn test_revoting() {
    let mut app = App::default();
    let govmod_id = app.store_code(proposal_contract());
    let core_addr = instantiate_with_staked_balances_governance(
        &mut app,
        govmod_id,
        to_binary(&InstantiateMsg {
            min_voting_period: None,
            max_voting_period: Duration::Height(6),
            only_members_execute: false,
            allow_revoting: true,
            deposit_info: None,
            voting_strategy: VotingStrategy::SingleChoice {
                quorum: PercentageThreshold::Majority {},
            },
            close_proposal_on_execution_failure: false,
        })
        .unwrap(),
        Some(vec![
            Cw20Coin {
                address: "a-1".to_string(),
                amount: Uint128::new(100_000_000),
            },
            Cw20Coin {
                address: "a-2".to_string(),
                amount: Uint128::new(100_000_000),
            },
        ]),
    );

    let governance_modules: Vec<ProposalModule> = app
        .wrap()
        .query_wasm_smart(
            core_addr,
            &cw_core::msg::QueryMsg::ProposalModules {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    let proposal_module = governance_modules.into_iter().next().unwrap().address;

    let options = vec![
        MultipleChoiceOption {
            description: "multiple choice option 1".to_string(),
            msgs: None,
        },
        MultipleChoiceOption {
            description: "multiple choice option 2".to_string(),
            msgs: None,
        },
    ];
    let mc_options = MultipleChoiceOptions { options };

    // Create a basic proposal with 2 options
    app.execute_contract(
        Addr::unchecked("a-1"),
        proposal_module.clone(),
        &ExecuteMsg::Propose {
            title: "A simple text proposal".to_string(),
            description: "A simple text proposal".to_string(),
            choices: mc_options,
        },
        &[],
    )
    .unwrap();

    // a-1 votes, vote_weights: [100_000_000, 0]
    app.execute_contract(
        Addr::unchecked("a-1"),
        proposal_module.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 1,
            vote: MultipleChoiceVote { option_id: 0 },
        },
        &[],
    )
    .unwrap();

    // a-2 votes, vote_weights: [100_000_000, 100_000_000]
    app.execute_contract(
        Addr::unchecked("a-2"),
        proposal_module.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 1,
            vote: MultipleChoiceVote { option_id: 1 },
        },
        &[],
    )
    .unwrap();

    // Time passes..
    app.update_block(|b| b.height += 2);

    // Assert that both vote options have equal vote weights at some block
    let proposal: ProposalResponse = app
        .wrap()
        .query_wasm_smart(
            proposal_module.clone(),
            &QueryMsg::Proposal { proposal_id: 1 },
        )
        .unwrap();
    assert_eq!(proposal.proposal.status, Status::Open);
    assert_eq!(
        proposal.proposal.votes.vote_weights[0],
        Uint128::new(100_000_000),
    );
    assert_eq!(
        proposal.proposal.votes.vote_weights[1],
        Uint128::new(100_000_000),
    );

    // More time passes..
    app.update_block(|b| b.height += 3);

    // Last moment a-2 has a change of mind,
    // votes shift to [200_000_000, 0]
    app.execute_contract(
        Addr::unchecked("a-2"),
        proposal_module.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 1,
            vote: MultipleChoiceVote { option_id: 0 },
        },
        &[],
    )
    .unwrap();

    app.update_block(next_block);

    // Assert that revote succeeded
    let proposal: ProposalResponse = app
        .wrap()
        .query_wasm_smart(proposal_module, &QueryMsg::Proposal { proposal_id: 1 })
        .unwrap();
    assert_eq!(proposal.proposal.status, Status::Passed);
    assert_eq!(
        proposal.proposal.votes.vote_weights[0],
        Uint128::new(200_000_000),
    );
    assert_eq!(proposal.proposal.votes.vote_weights[1], Uint128::new(0),);
}

/// Tests that revoting is stored at a per-proposal level.
/// Proposals created while revoting is enabled should not
/// have it disabled if a config change turns if off.
#[test]
fn test_allow_revoting_config_changes() {
    let mut app = App::default();
    let govmod_id = app.store_code(proposal_contract());
    let core_addr = instantiate_with_staked_balances_governance(
        &mut app,
        govmod_id,
        to_binary(&InstantiateMsg {
            min_voting_period: None,
            max_voting_period: Duration::Height(6),
            only_members_execute: false,
            allow_revoting: true,
            deposit_info: None,
            voting_strategy: VotingStrategy::SingleChoice {
                quorum: PercentageThreshold::Majority {},
            },
            close_proposal_on_execution_failure: false,
        })
        .unwrap(),
        Some(vec![
            Cw20Coin {
                address: "a-1".to_string(),
                amount: Uint128::new(100_000_000),
            },
            Cw20Coin {
                address: "a-2".to_string(),
                amount: Uint128::new(100_000_000),
            },
        ]),
    );

    let governance_modules: Vec<ProposalModule> = app
        .wrap()
        .query_wasm_smart(
            core_addr.clone(),
            &cw_core::msg::QueryMsg::ProposalModules {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    let proposal_module = governance_modules.into_iter().next().unwrap().address;

    let options = vec![
        MultipleChoiceOption {
            description: "multiple choice option 1".to_string(),
            msgs: None,
        },
        MultipleChoiceOption {
            description: "multiple choice option 2".to_string(),
            msgs: None,
        },
    ];
    let mc_options = MultipleChoiceOptions { options };

    // Create a basic proposal with 2 options that allows revoting
    app.execute_contract(
        Addr::unchecked("a-1"),
        proposal_module.clone(),
        &ExecuteMsg::Propose {
            title: "A simple text proposal".to_string(),
            description: "A simple text proposal".to_string(),
            choices: mc_options.clone(),
        },
        &[],
    )
    .unwrap();

    // Disable revoting
    app.execute_contract(
        core_addr.clone(),
        proposal_module.clone(),
        &ExecuteMsg::UpdateConfig {
            min_voting_period: None,
            max_voting_period: Duration::Height(6),
            only_members_execute: false,
            allow_revoting: false,
            dao: core_addr.to_string(),
            deposit_info: None,
            voting_strategy: VotingStrategy::SingleChoice {
                quorum: PercentageThreshold::Majority {},
            },
            close_proposal_on_execution_failure: false,
        },
        &[],
    )
    .unwrap();

    // Assert that proposal_id: 1 still allows revoting
    let proposal: ProposalResponse = app
        .wrap()
        .query_wasm_smart(
            proposal_module.clone(),
            &QueryMsg::Proposal { proposal_id: 1 },
        )
        .unwrap();
    assert!(proposal.proposal.allow_revoting);

    app.execute_contract(
        Addr::unchecked("a-1"),
        proposal_module.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 1,
            vote: MultipleChoiceVote { option_id: 0 },
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        Addr::unchecked("a-1"),
        proposal_module.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 1,
            vote: MultipleChoiceVote { option_id: 1 },
        },
        &[],
    )
    .unwrap();

    // New proposals should not allow revoting
    app.execute_contract(
        Addr::unchecked("a-2"),
        proposal_module.clone(),
        &ExecuteMsg::Propose {
            title: "A very complex text proposal".to_string(),
            description: "A very complex text proposal".to_string(),
            choices: mc_options,
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        Addr::unchecked("a-2"),
        proposal_module.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 2,
            vote: MultipleChoiceVote { option_id: 0 },
        },
        &[],
    )
    .unwrap();

    let err: ContractError = app
        .execute_contract(
            Addr::unchecked("a-2"),
            proposal_module,
            &ExecuteMsg::Vote {
                proposal_id: 2,
                vote: MultipleChoiceVote { option_id: 1 },
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    assert!(matches!(err, ContractError::AlreadyVoted {}));
}

/// Tests that we error if a revote casts the same vote as the
/// previous vote.
#[test]
fn test_revoting_same_vote_twice() {
    let mut app = App::default();
    let govmod_id = app.store_code(proposal_contract());
    let core_addr = instantiate_with_staked_balances_governance(
        &mut app,
        govmod_id,
        to_binary(&InstantiateMsg {
            min_voting_period: None,
            max_voting_period: Duration::Height(6),
            only_members_execute: false,
            allow_revoting: true,
            deposit_info: None,
            voting_strategy: VotingStrategy::SingleChoice {
                quorum: PercentageThreshold::Majority {},
            },
            close_proposal_on_execution_failure: false,
        })
        .unwrap(),
        Some(vec![
            Cw20Coin {
                address: "a-1".to_string(),
                amount: Uint128::new(100_000_000),
            },
            Cw20Coin {
                address: "a-2".to_string(),
                amount: Uint128::new(100_000_000),
            },
        ]),
    );

    let governance_modules: Vec<ProposalModule> = app
        .wrap()
        .query_wasm_smart(
            core_addr,
            &cw_core::msg::QueryMsg::ProposalModules {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    let proposal_module = governance_modules.into_iter().next().unwrap().address;

    let options = vec![
        MultipleChoiceOption {
            description: "multiple choice option 1".to_string(),
            msgs: None,
        },
        MultipleChoiceOption {
            description: "multiple choice option 2".to_string(),
            msgs: None,
        },
    ];
    let mc_options = MultipleChoiceOptions { options };

    // Create a basic proposal with 2 options that allows revoting
    app.execute_contract(
        Addr::unchecked("a-1"),
        proposal_module.clone(),
        &ExecuteMsg::Propose {
            title: "A simple text proposal".to_string(),
            description: "A simple text proposal".to_string(),
            choices: mc_options,
        },
        &[],
    )
    .unwrap();

    // Cast a vote
    app.execute_contract(
        Addr::unchecked("a-1"),
        proposal_module.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 1,
            vote: MultipleChoiceVote { option_id: 0 },
        },
        &[],
    )
    .unwrap();

    // Revote for the same option as currently voted
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked("a-1"),
            proposal_module,
            &ExecuteMsg::Vote {
                proposal_id: 1,
                vote: MultipleChoiceVote { option_id: 0 },
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    // Can't cast the same vote twice.
    assert!(matches!(err, ContractError::AlreadyCast {}));
}

/// Tests that revoting into a non-existing vote option
/// does not invalidate the initial vote
#[test]
fn test_invalid_revote_does_not_invalidate_initial_vote() {
    let mut app = App::default();
    let govmod_id = app.store_code(proposal_contract());
    let core_addr = instantiate_with_staked_balances_governance(
        &mut app,
        govmod_id,
        to_binary(&InstantiateMsg {
            min_voting_period: None,
            max_voting_period: Duration::Height(6),
            only_members_execute: false,
            allow_revoting: true,
            deposit_info: None,
            voting_strategy: VotingStrategy::SingleChoice {
                quorum: PercentageThreshold::Majority {},
            },
            close_proposal_on_execution_failure: false,
        })
        .unwrap(),
        Some(vec![
            Cw20Coin {
                address: "a-1".to_string(),
                amount: Uint128::new(100_000_000),
            },
            Cw20Coin {
                address: "a-2".to_string(),
                amount: Uint128::new(100_000_000),
            },
        ]),
    );

    let governance_modules: Vec<ProposalModule> = app
        .wrap()
        .query_wasm_smart(
            core_addr,
            &cw_core::msg::QueryMsg::ProposalModules {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    let proposal_module = governance_modules.into_iter().next().unwrap().address;

    let options = vec![
        MultipleChoiceOption {
            description: "multiple choice option 1".to_string(),
            msgs: None,
        },
        MultipleChoiceOption {
            description: "multiple choice option 2".to_string(),
            msgs: None,
        },
    ];
    let mc_options = MultipleChoiceOptions { options };

    // Create a basic proposal with 2 options
    app.execute_contract(
        Addr::unchecked("a-1"),
        proposal_module.clone(),
        &ExecuteMsg::Propose {
            title: "A simple text proposal".to_string(),
            description: "A simple text proposal".to_string(),
            choices: mc_options,
        },
        &[],
    )
    .unwrap();

    // a-1 votes, vote_weights: [100_000_000, 0]
    app.execute_contract(
        Addr::unchecked("a-1"),
        proposal_module.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 1,
            vote: MultipleChoiceVote { option_id: 0 },
        },
        &[],
    )
    .unwrap();

    // a-2 votes, vote_weights: [100_000_000, 100_000_000]
    app.execute_contract(
        Addr::unchecked("a-2"),
        proposal_module.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 1,
            vote: MultipleChoiceVote { option_id: 1 },
        },
        &[],
    )
    .unwrap();

    app.update_block(next_block);

    // Assert that both vote options have equal vote weights at some block
    let proposal: ProposalResponse = app
        .wrap()
        .query_wasm_smart(
            proposal_module.clone(),
            &QueryMsg::Proposal { proposal_id: 1 },
        )
        .unwrap();
    assert_eq!(proposal.proposal.status, Status::Open);
    assert_eq!(
        proposal.proposal.votes.vote_weights[0],
        Uint128::new(100_000_000),
    );
    assert_eq!(
        proposal.proposal.votes.vote_weights[1],
        Uint128::new(100_000_000),
    );

    // Time passes..
    app.update_block(|b| b.height += 3);

    // Last moment a-2 has a change of mind and attempts
    // to vote for a non-existing option
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked("a-2"),
            proposal_module,
            &ExecuteMsg::Vote {
                proposal_id: 1,
                vote: MultipleChoiceVote { option_id: 99 },
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    // Assert that prior votes remained the same
    assert_eq!(
        proposal.proposal.votes.vote_weights[0],
        Uint128::new(100_000_000),
    );
    assert_eq!(
        proposal.proposal.votes.vote_weights[1],
        Uint128::new(100_000_000),
    );
    assert!(matches!(err, ContractError::InvalidVote {}));
}

#[test]
fn test_return_deposit_to_dao_on_proposal_failure() {
    let (mut app, core_addr) = do_test_votes_cw20_balances(
        vec![TestMultipleChoiceVote {
            voter: "blue".to_string(),
            position: MultipleChoiceVote { option_id: 2 },
            weight: Uint128::new(10),
            should_execute: ShouldExecute::Yes,
        }],
        VotingStrategy::SingleChoice {
            quorum: PercentageThreshold::Majority {},
        },
        Status::Open,
        Some(Uint128::new(100)),
        Some(DepositInfo {
            token: DepositToken::VotingModuleToken {},
            deposit: Uint128::new(1),
            refund_failed_proposals: false,
        }),
        false,
    );

    let core_state: cw_core::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(core_addr.clone(), &cw_core::msg::QueryMsg::DumpState {})
        .unwrap();
    let proposal_modules = core_state.proposal_modules;

    assert_eq!(proposal_modules.len(), 1);
    let proposal_multiple = proposal_modules.into_iter().next().unwrap().address;

    // Make the proposal expire. It has now failed.
    app.update_block(|block| block.height += 10);

    // Close the proposal, this should work as the proposal is now
    // open and expired.
    app.execute_contract(
        Addr::unchecked("keze"),
        proposal_multiple.clone(),
        &ExecuteMsg::Close { proposal_id: 1 },
        &[],
    )
    .unwrap();

    // Check that a refund was issued to the DAO.
    let proposal_config: Config = app
        .wrap()
        .query_wasm_smart(proposal_multiple, &QueryMsg::Config {})
        .unwrap();
    let CheckedDepositInfo { token, .. } = proposal_config.deposit_info.unwrap();
    let balance: cw20::BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            token,
            &cw20::Cw20QueryMsg::Balance {
                address: core_addr.into_string(),
            },
        )
        .unwrap();

    // Deposit should now belong to the DAO.
    assert_eq!(balance.balance, Uint128::new(1));
}

#[test]
fn test_close_failed_proposal() {
    let mut app = App::default();
    let govmod_id = app.store_code(proposal_contract());

    let quorum = PercentageThreshold::Majority {};
    let voting_strategy = VotingStrategy::SingleChoice { quorum };
    let max_voting_period = cw_utils::Duration::Height(6);
    let instantiate = InstantiateMsg {
        max_voting_period,
        voting_strategy,
        min_voting_period: None,
        only_members_execute: false,
        allow_revoting: false,
        deposit_info: None,
        close_proposal_on_execution_failure: true,
    };

    let governance_addr = instantiate_with_staking_active_threshold(
        &mut app,
        govmod_id,
        to_binary(&instantiate).unwrap(),
        None,
        None,
    );
    let governance_modules: Vec<ProposalModule> = app
        .wrap()
        .query_wasm_smart(
            governance_addr,
            &cw_core::msg::QueryMsg::ProposalModules {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(governance_modules.len(), 1);
    let govmod_single = governance_modules.into_iter().next().unwrap().address;

    let govmod_config: Config = app
        .wrap()
        .query_wasm_smart(govmod_single.clone(), &QueryMsg::Config {})
        .unwrap();
    let dao = govmod_config.dao;
    let voting_module: Addr = app
        .wrap()
        .query_wasm_smart(dao, &cw_core::msg::QueryMsg::VotingModule {})
        .unwrap();
    let staking_contract: Addr = app
        .wrap()
        .query_wasm_smart(
            voting_module.clone(),
            &cw20_staked_balance_voting::msg::QueryMsg::StakingContract {},
        )
        .unwrap();
    let token_contract: Addr = app
        .wrap()
        .query_wasm_smart(
            voting_module,
            &cw_core_interface::voting::Query::TokenContract {},
        )
        .unwrap();

    // Stake some tokens so we can propose
    let msg = cw20::Cw20ExecuteMsg::Send {
        contract: staking_contract.to_string(),
        amount: Uint128::new(2000),
        msg: to_binary(&cw20_stake::msg::ReceiveMsg::Stake {}).unwrap(),
    };
    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        token_contract.clone(),
        &msg,
        &[],
    )
    .unwrap();
    app.update_block(next_block);

    let msg = cw20::Cw20ExecuteMsg::Burn {
        amount: Uint128::new(2000),
    };
    let binary_msg = to_binary(&msg).unwrap();

    let options = vec![
        MultipleChoiceOption {
            description: "Burn or burn".to_string(),
            msgs: Some(vec![WasmMsg::Execute {
                contract_addr: token_contract.to_string(),
                msg: binary_msg,
                funds: vec![],
            }
            .into()]),
        },
        MultipleChoiceOption {
            description: "Don't burn".to_string(),
            msgs: None,
        },
    ];

    let mc_options = MultipleChoiceOptions { options };

    // Overburn tokens
    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        govmod_single.clone(),
        &ExecuteMsg::Propose {
            title: "A simple burn tokens proposal".to_string(),
            description: "Burning more tokens, than dao treasury have".to_string(),
            choices: mc_options.clone(),
        },
        &[],
    )
    .unwrap();

    // Vote on proposal
    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        govmod_single.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 1,
            vote: MultipleChoiceVote { option_id: 0 },
        },
        &[],
    )
    .unwrap();

    // Update block
    let timestamp = Timestamp::from_seconds(300_000_000);
    app.update_block(|block| block.time = timestamp);

    // Execute proposal
    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        govmod_single.clone(),
        &ExecuteMsg::Execute { proposal_id: 1 },
        &[],
    )
    .unwrap();

    let failed: ProposalResponse = app
        .wrap()
        .query_wasm_smart(
            govmod_single.clone(),
            &QueryMsg::Proposal { proposal_id: 1 },
        )
        .unwrap();

    assert_eq!(failed.proposal.status, Status::ExecutionFailed);
    assert_eq!(failed.proposal.last_updated, app.block_info().time);
    // With disabled feature
    // Disable feature first
    {
        let original: Config = app
            .wrap()
            .query_wasm_smart(govmod_single.clone(), &QueryMsg::Config {})
            .unwrap();

        app.execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            govmod_single.clone(),
            &ExecuteMsg::Propose {
                title: "Disable closing failed proposals".to_string(),
                description: "We want to re-execute failed proposals".to_string(),
                choices: MultipleChoiceOptions {
                    options: vec![
                        MultipleChoiceOption {
                            description: "Disable closing failed proposals".to_string(),
                            msgs: Some(vec![WasmMsg::Execute {
                                contract_addr: govmod_single.to_string(),
                                msg: to_binary(&ExecuteMsg::UpdateConfig {
                                    voting_strategy: VotingStrategy::SingleChoice { quorum },
                                    max_voting_period: original.max_voting_period,
                                    min_voting_period: original.min_voting_period,
                                    only_members_execute: original.only_members_execute,
                                    allow_revoting: false,
                                    dao: original.dao.to_string(),
                                    deposit_info: None,
                                    close_proposal_on_execution_failure: false,
                                })
                                .unwrap(),
                                funds: vec![],
                            }
                            .into()]),
                        },
                        MultipleChoiceOption {
                            description: "Don't disable".to_string(),
                            msgs: None,
                        },
                    ],
                },
            },
            &[],
        )
        .unwrap();

        // Vote on proposal
        app.execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            govmod_single.clone(),
            &ExecuteMsg::Vote {
                proposal_id: 2,
                vote: MultipleChoiceVote { option_id: 0 },
            },
            &[],
        )
        .unwrap();

        // Execute proposal
        app.execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            govmod_single.clone(),
            &ExecuteMsg::Execute { proposal_id: 2 },
            &[],
        )
        .unwrap();
    }

    // Overburn tokens (again), this time without reverting
    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        govmod_single.clone(),
        &ExecuteMsg::Propose {
            title: "A simple burn tokens proposal".to_string(),
            description: "Burning more tokens, than dao treasury have".to_string(),
            choices: mc_options,
        },
        &[],
    )
    .unwrap();

    // Vote on proposal
    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        govmod_single.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 3,
            vote: MultipleChoiceVote { option_id: 0 },
        },
        &[],
    )
    .unwrap();

    // Execute proposal
    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        govmod_single.clone(),
        &ExecuteMsg::Execute { proposal_id: 3 },
        &[],
    )
    .expect_err("Should be sub overflow");

    // Status should still be passed
    let updated: ProposalResponse = app
        .wrap()
        .query_wasm_smart(govmod_single, &QueryMsg::Proposal { proposal_id: 3 })
        .unwrap();

    // not reverted
    assert_eq!(updated.proposal.status, Status::Passed);
}

#[test]
fn test_no_double_refund_on_execute_fail_and_close() {
    let mut app = App::default();
    let proposal_module_id = app.store_code(proposal_contract());

    let voting_strategy = VotingStrategy::SingleChoice {
        quorum: PercentageThreshold::Majority {},
    };
    let max_voting_period = cw_utils::Duration::Height(6);
    let instantiate = InstantiateMsg {
        voting_strategy,
        max_voting_period,
        min_voting_period: None,
        only_members_execute: false,
        allow_revoting: false,
        deposit_info: Some(DepositInfo {
            token: DepositToken::VotingModuleToken {},
            deposit: Uint128::new(1),
            // Important to set to true here as we want to be sure
            // that we don't get a second refund on close. Refunds on
            // close only happen if this is true.
            refund_failed_proposals: true,
        }),
        close_proposal_on_execution_failure: true,
    };

    let core_addr = instantiate_with_staking_active_threshold(
        &mut app,
        proposal_module_id,
        to_binary(&instantiate).unwrap(),
        Some(vec![Cw20Coin {
            address: CREATOR_ADDR.to_string(),
            // One token for sending to the DAO treasury, one token
            // for staking, one token for paying the proposal deposit.
            amount: Uint128::new(3),
        }]),
        None,
    );
    let proposal_modules: Vec<ProposalModule> = app
        .wrap()
        .query_wasm_smart(
            core_addr,
            &cw_core::msg::QueryMsg::ProposalModules {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(proposal_modules.len(), 1);
    let proposal_single = proposal_modules.into_iter().next().unwrap().address;

    let proposal_config: Config = app
        .wrap()
        .query_wasm_smart(proposal_single.clone(), &QueryMsg::Config {})
        .unwrap();
    let dao = proposal_config.dao;
    let voting_module: Addr = app
        .wrap()
        .query_wasm_smart(dao, &cw_core::msg::QueryMsg::VotingModule {})
        .unwrap();
    let staking_contract: Addr = app
        .wrap()
        .query_wasm_smart(
            voting_module.clone(),
            &cw20_staked_balance_voting::msg::QueryMsg::StakingContract {},
        )
        .unwrap();
    let token_contract: Addr = app
        .wrap()
        .query_wasm_smart(
            voting_module,
            &cw_core_interface::voting::Query::TokenContract {},
        )
        .unwrap();

    // Stake a token so we can propose.
    let msg = cw20::Cw20ExecuteMsg::Send {
        contract: staking_contract.to_string(),
        amount: Uint128::new(1),
        msg: to_binary(&cw20_stake::msg::ReceiveMsg::Stake {}).unwrap(),
    };
    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        token_contract.clone(),
        &msg,
        &[],
    )
    .unwrap();
    app.update_block(next_block);

    // Send some tokens to the proposal module so it has the ability
    // to double refund if the code is buggy.
    let msg = cw20::Cw20ExecuteMsg::Transfer {
        recipient: proposal_single.to_string(),
        amount: Uint128::new(1),
    };
    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        token_contract.clone(),
        &msg,
        &[],
    )
    .unwrap();

    let msg = cw20::Cw20ExecuteMsg::Burn {
        amount: Uint128::new(2000),
    };
    let binary_msg = to_binary(&msg).unwrap();

    // Increase allowance to pay the proposal deposit.
    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        token_contract.clone(),
        &cw20_base::msg::ExecuteMsg::IncreaseAllowance {
            spender: proposal_single.to_string(),
            amount: Uint128::new(1),
            expires: None,
        },
        &[],
    )
    .unwrap();

    let choices = MultipleChoiceOptions {
        options: vec![
            MultipleChoiceOption {
                description: "Burning more tokens, than dao treasury have".to_string(),
                msgs: Some(vec![WasmMsg::Execute {
                    contract_addr: token_contract.to_string(),
                    msg: binary_msg,
                    funds: vec![],
                }
                .into()]),
            },
            MultipleChoiceOption {
                description: "hi there".to_string(),
                msgs: None,
            },
        ],
    };
    // proposal to overburn tokens
    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        proposal_single.clone(),
        &ExecuteMsg::Propose {
            title: "A simple burn tokens proposal".to_string(),
            description: "Burning more tokens, than dao treasury have".to_string(),
            choices,
        },
        &[],
    )
    .unwrap();

    // Vote on proposal
    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        proposal_single.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 1,
            vote: MultipleChoiceVote { option_id: 0 },
        },
        &[],
    )
    .unwrap();

    // Update block
    let timestamp = Timestamp::from_seconds(300_000_000);
    app.update_block(|block| block.time = timestamp);

    // Execute proposal
    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        proposal_single.clone(),
        &ExecuteMsg::Execute { proposal_id: 1 },
        &[],
    )
    .unwrap();

    let failed: ProposalResponse = app
        .wrap()
        .query_wasm_smart(
            proposal_single.clone(),
            &QueryMsg::Proposal { proposal_id: 1 },
        )
        .unwrap();

    assert_eq!(failed.proposal.status, Status::ExecutionFailed);
    assert_eq!(failed.proposal.last_updated, app.block_info().time);

    // Check that our deposit has been refunded.
    let balance: cw20::BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            token_contract.to_string(),
            &cw20::Cw20QueryMsg::Balance {
                address: CREATOR_ADDR.to_string(),
            },
        )
        .unwrap();

    assert_eq!(balance.balance, Uint128::new(1));

    // Close the proposal - this should fail as it was executed.
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            proposal_single,
            &ExecuteMsg::Close { proposal_id: 1 },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    assert!(matches!(err, ContractError::WrongCloseStatus {}));

    // Check that our deposit was not refunded a second time on close.
    let balance: cw20::BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            token_contract.to_string(),
            &cw20::Cw20QueryMsg::Balance {
                address: CREATOR_ADDR.to_string(),
            },
        )
        .unwrap();

    assert_eq!(balance.balance, Uint128::new(1));
}

#[test]
fn test_timestamp_updated() {
    let mut app = App::default();
    let govmod_id = app.store_code(proposal_contract());

    let voting_strategy = VotingStrategy::SingleChoice {
        quorum: PercentageThreshold::Majority {},
    };
    let max_voting_period = cw_utils::Duration::Height(6);
    let instantiate = InstantiateMsg {
        voting_strategy,
        max_voting_period,
        min_voting_period: None,
        only_members_execute: false,
        deposit_info: None,
        close_proposal_on_execution_failure: true,
        allow_revoting: false,
    };

    let governance_addr = instantiate_with_cw20_balances_governance(
        &mut app,
        govmod_id,
        to_binary(&instantiate).unwrap(),
        Some(vec![
            Cw20Coin {
                address: "voter".to_string(),
                amount: Uint128::new(3),
            },
            Cw20Coin {
                address: "voter2".to_string(),
                amount: Uint128::new(2),
            },
        ]),
    );

    let governance_modules: Vec<ProposalModule> = app
        .wrap()
        .query_wasm_smart(
            governance_addr,
            &cw_core::msg::QueryMsg::ProposalModules {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();

    let govmod_single = governance_modules.into_iter().next().unwrap().address;

    let options = vec![
        MultipleChoiceOption {
            description: "Option 1".to_string(),
            msgs: None,
        },
        MultipleChoiceOption {
            description: "Option 2".to_string(),
            msgs: None,
        },
    ];

    let mc_options = MultipleChoiceOptions { options };

    // Create 2 proposals.
    app.execute_contract(
        Addr::unchecked("voter"),
        govmod_single.clone(),
        &ExecuteMsg::Propose {
            title: "A simple text proposal".to_string(),
            description: "This is a simple text proposal".to_string(),
            choices: mc_options.clone(),
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        Addr::unchecked("voter"),
        govmod_single.clone(),
        &ExecuteMsg::Propose {
            title: "A simple text proposal".to_string(),
            description: "This is a simple text proposal".to_string(),
            choices: mc_options,
        },
        &[],
    )
    .unwrap();

    let created_1: ProposalResponse = app
        .wrap()
        .query_wasm_smart(
            govmod_single.clone(),
            &QueryMsg::Proposal { proposal_id: 1 },
        )
        .unwrap();
    let current_block = app.block_info();

    // Verify created and last updated
    assert_eq!(created_1.proposal.created, current_block.time);
    assert_eq!(created_1.proposal.last_updated, current_block.time);

    let created_2: ProposalResponse = app
        .wrap()
        .query_wasm_smart(
            govmod_single.clone(),
            &QueryMsg::Proposal { proposal_id: 2 },
        )
        .unwrap();

    // Verify created and last updated
    assert_eq!(created_2.proposal.created, current_block.time);
    assert_eq!(created_2.proposal.last_updated, current_block.time);

    // Update block
    let timestamp = Timestamp::from_seconds(300_000_000);
    app.update_block(|block| block.time = timestamp);

    // Vote on proposal
    app.execute_contract(
        Addr::unchecked("voter"),
        govmod_single.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 1,
            vote: MultipleChoiceVote { option_id: 0 },
        },
        &[],
    )
    .unwrap();

    // Expect that last_updated changed because of status change
    let updated: ProposalResponse = app
        .wrap()
        .query_wasm_smart(
            govmod_single.clone(),
            &QueryMsg::Proposal { proposal_id: 1 },
        )
        .unwrap();

    assert_eq!(updated.proposal.last_updated, app.block_info().time);
    assert_eq!(updated.proposal.status, Status::Passed);

    // Update block
    let timestamp = Timestamp::from_seconds(500_000_000);
    app.update_block(|block| block.time = timestamp);
    let latest_time = app.block_info().time;

    // Execute proposal
    app.execute_contract(
        Addr::unchecked("voter"),
        govmod_single.clone(),
        &ExecuteMsg::Execute { proposal_id: 1 },
        &[],
    )
    .unwrap();

    // Status should have changed to 'Executed'
    let updated: ProposalResponse = app
        .wrap()
        .query_wasm_smart(
            govmod_single.clone(),
            &QueryMsg::Proposal { proposal_id: 1 },
        )
        .unwrap();

    assert_eq!(updated.proposal.last_updated, latest_time);
    assert_eq!(updated.proposal.status, Status::Executed);

    let timestamp = Timestamp::from_seconds(700_000_000);
    app.update_block(|block| block.time = timestamp);
    let latest_time = app.block_info().time;

    // Vote no on second proposal
    app.execute_contract(
        Addr::unchecked("voter"),
        govmod_single.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 2,
            vote: MultipleChoiceVote { option_id: 2 },
        },
        &[],
    )
    .unwrap();

    // Status should have changed to 'Rejected'
    let updated: ProposalResponse = app
        .wrap()
        .query_wasm_smart(
            govmod_single.clone(),
            &QueryMsg::Proposal { proposal_id: 2 },
        )
        .unwrap();

    assert_eq!(updated.proposal.last_updated, latest_time);
    assert_eq!(updated.proposal.status, Status::Rejected);

    let timestamp = Timestamp::from_seconds(900_000_000);
    app.update_block(|block| block.time = timestamp);
    let latest_time = app.block_info().time;

    // Close second proposal
    app.execute_contract(
        Addr::unchecked("voter"),
        govmod_single.clone(),
        &ExecuteMsg::Close { proposal_id: 2 },
        &[],
    )
    .unwrap();

    // Status should have changed to 'Closed'
    let updated: ProposalResponse = app
        .wrap()
        .query_wasm_smart(govmod_single, &QueryMsg::Proposal { proposal_id: 2 })
        .unwrap();

    assert_eq!(updated.proposal.last_updated, latest_time);
    assert_eq!(updated.proposal.status, Status::Closed);
}

#[test]
fn test_find_proposals() {
    let mut app = App::default();
    let govmod_id = app.store_code(proposal_contract());

    let quorum = PercentageThreshold::Majority {};
    let voting_strategy = VotingStrategy::SingleChoice { quorum };
    let max_voting_period = cw_utils::Duration::Height(6);
    let instantiate = InstantiateMsg {
        min_voting_period: None,
        max_voting_period,
        only_members_execute: false,
        deposit_info: None,
        voting_strategy,
        allow_revoting: false,
        close_proposal_on_execution_failure: true,
    };

    let governance_addr = instantiate_with_staking_active_threshold(
        &mut app,
        govmod_id,
        to_binary(&instantiate).unwrap(),
        Some(vec![
            Cw20Coin {
                address: "one".to_string(),
                amount: Uint128::new(1),
            },
            Cw20Coin {
                address: "two".to_string(),
                amount: Uint128::new(1),
            },
            Cw20Coin {
                address: "three".to_string(),
                amount: Uint128::new(1),
            },
            Cw20Coin {
                address: "four".to_string(),
                amount: Uint128::new(1),
            },
            Cw20Coin {
                address: "five".to_string(),
                amount: Uint128::new(1),
            },
        ]),
        None,
    );

    let governance_modules: Vec<ProposalModule> = app
        .wrap()
        .query_wasm_smart(
            governance_addr,
            &cw_core::msg::QueryMsg::ProposalModules {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(governance_modules.len(), 1);
    let govmod_single = governance_modules.into_iter().next().unwrap().address;

    let options = vec![
        MultipleChoiceOption {
            description: "Yes".to_string(),
            msgs: None,
        },
        MultipleChoiceOption {
            description: "No".to_string(),
            msgs: None,
        },
    ];

    let mc_options = MultipleChoiceOptions { options };

    let govmod_config: Config = app
        .wrap()
        .query_wasm_smart(govmod_single.clone(), &QueryMsg::Config {})
        .unwrap();
    let dao = govmod_config.dao;
    let voting_module: Addr = app
        .wrap()
        .query_wasm_smart(dao, &cw_core::msg::QueryMsg::VotingModule {})
        .unwrap();
    let staking_contract: Addr = app
        .wrap()
        .query_wasm_smart(
            voting_module.clone(),
            &cw20_staked_balance_voting::msg::QueryMsg::StakingContract {},
        )
        .unwrap();
    let token_contract: Addr = app
        .wrap()
        .query_wasm_smart(
            voting_module,
            &cw_core_interface::voting::Query::TokenContract {},
        )
        .unwrap();

    // Stake some tokens so we can propose and vote
    let msg = cw20::Cw20ExecuteMsg::Send {
        contract: staking_contract.to_string(),
        amount: Uint128::new(1),
        msg: to_binary(&cw20_stake::msg::ReceiveMsg::Stake {}).unwrap(),
    };
    app.execute_contract(Addr::unchecked("one"), token_contract.clone(), &msg, &[])
        .unwrap();
    app.execute_contract(Addr::unchecked("two"), token_contract.clone(), &msg, &[])
        .unwrap();
    app.execute_contract(Addr::unchecked("three"), token_contract.clone(), &msg, &[])
        .unwrap();
    app.execute_contract(Addr::unchecked("four"), token_contract.clone(), &msg, &[])
        .unwrap();
    app.execute_contract(Addr::unchecked("five"), token_contract, &msg, &[])
        .unwrap();
    app.update_block(next_block);

    app.execute_contract(
        Addr::unchecked("one"),
        govmod_single.clone(),
        &ExecuteMsg::Propose {
            title: "Propose a thing.".to_string(),
            description: "Do the thing.".to_string(),
            choices: mc_options.clone(),
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        Addr::unchecked("one"),
        govmod_single.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 1,
            vote: MultipleChoiceVote { option_id: 0 },
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        Addr::unchecked("two"),
        govmod_single.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 1,
            vote: MultipleChoiceVote { option_id: 1 },
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        Addr::unchecked("one"),
        govmod_single.clone(),
        &ExecuteMsg::Propose {
            title: "Propose a thing.".to_string(),
            description: "Do the thing.".to_string(),
            choices: mc_options.clone(),
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        Addr::unchecked("one"),
        govmod_single.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 2,
            vote: MultipleChoiceVote { option_id: 1 },
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        Addr::unchecked("two"),
        govmod_single.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 2,
            vote: MultipleChoiceVote { option_id: 0 },
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        Addr::unchecked("three"),
        govmod_single.clone(),
        &ExecuteMsg::Propose {
            title: "Propose a thing.".to_string(),
            description: "Do the thing.".to_string(),
            choices: mc_options,
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        Addr::unchecked("one"),
        govmod_single.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 3,
            vote: MultipleChoiceVote { option_id: 0 },
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        Addr::unchecked("two"),
        govmod_single.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 3,
            vote: MultipleChoiceVote { option_id: 0 },
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        Addr::unchecked("three"),
        govmod_single.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 3,
            vote: MultipleChoiceVote { option_id: 0 },
        },
        &[],
    )
    .unwrap();

    app.update_block(|b| b.height += 10);

    app.execute_contract(
        Addr::unchecked("four"),
        govmod_single.clone(),
        &ExecuteMsg::Execute { proposal_id: 3 },
        &[],
    )
    .unwrap();

    // Testing search
    // wallet "one" voted options on proposals:
    // 1 - "0"
    // 2 - "1"
    // 3 - "0"
    // and 3rd proposal is executed
    {
        let answ: FilterListProposalsResponse = app
            .wrap()
            .query_wasm_smart(
                govmod_single.clone(),
                &QueryMsg::FilterListProposals {
                    wallet: "one".into(),
                    status: None,
                    wallet_vote: WalletVote::AnyVote {},
                    start_after: None,
                    limit: None,
                },
            )
            .unwrap();
        assert_eq!(
            answ.proposals
                .into_iter()
                .map(|p| p.id)
                .collect::<Vec<u64>>(),
            [1, 2, 3]
        );
        assert_eq!(answ.last_proposal_id, 3);

        let answ: FilterListProposalsResponse = app
            .wrap()
            .query_wasm_smart(
                govmod_single.clone(),
                &QueryMsg::FilterListProposals {
                    wallet: "one".into(),
                    status: Some(Status::Open),
                    wallet_vote: WalletVote::AnyVote {},
                    start_after: None,
                    limit: None,
                },
            )
            .unwrap();
        assert_eq!(
            answ.proposals
                .into_iter()
                .map(|p| p.id)
                .collect::<Vec<u64>>(),
            [1, 2]
        );
        assert_eq!(answ.last_proposal_id, 3);

        let answ: FilterListProposalsResponse = app
            .wrap()
            .query_wasm_smart(
                govmod_single.clone(),
                &QueryMsg::FilterListProposals {
                    wallet: "one".into(),
                    status: Some(Status::Executed),
                    wallet_vote: WalletVote::AnyVote {},
                    start_after: None,
                    limit: None,
                },
            )
            .unwrap();
        assert_eq!(
            answ.proposals
                .into_iter()
                .map(|p| p.id)
                .collect::<Vec<u64>>(),
            [3]
        );
        assert_eq!(answ.last_proposal_id, 3);

        let answ: FilterListProposalsResponse = app
            .wrap()
            .query_wasm_smart(
                govmod_single.clone(),
                &QueryMsg::FilterListProposals {
                    wallet: "one".into(),
                    status: None,
                    wallet_vote: WalletVote::Voted(MultipleChoiceVote { option_id: 0 }),
                    start_after: None,
                    limit: None,
                },
            )
            .unwrap();
        assert_eq!(
            answ.proposals
                .into_iter()
                .map(|p| p.id)
                .collect::<Vec<u64>>(),
            [1, 3]
        );
        assert_eq!(answ.last_proposal_id, 3);

        let answ: FilterListProposalsResponse = app
            .wrap()
            .query_wasm_smart(
                govmod_single.clone(),
                &QueryMsg::FilterListProposals {
                    wallet: "one".into(),
                    status: Some(Status::Open),
                    wallet_vote: WalletVote::Voted(MultipleChoiceVote { option_id: 0 }),
                    start_after: None,
                    limit: None,
                },
            )
            .unwrap();
        assert_eq!(
            answ.proposals
                .into_iter()
                .map(|p| p.id)
                .collect::<Vec<u64>>(),
            [1]
        );
        assert_eq!(answ.last_proposal_id, 3);
    }

    // Pagination test
    {
        let answ: FilterListProposalsResponse = app
            .wrap()
            .query_wasm_smart(
                govmod_single.clone(),
                &QueryMsg::FilterListProposals {
                    wallet: "one".into(),
                    status: None,
                    wallet_vote: WalletVote::AnyVote {},
                    start_after: Some(1),
                    limit: None,
                },
            )
            .unwrap();
        assert_eq!(
            answ.proposals
                .into_iter()
                .map(|p| p.id)
                .collect::<Vec<u64>>(),
            [2, 3]
        );
        assert_eq!(answ.last_proposal_id, 3);

        let answ: FilterListProposalsResponse = app
            .wrap()
            .query_wasm_smart(
                govmod_single.clone(),
                &QueryMsg::FilterListProposals {
                    wallet: "one".into(),
                    status: None,
                    wallet_vote: WalletVote::AnyVote {},
                    start_after: None,
                    limit: Some(2),
                },
            )
            .unwrap();
        assert_eq!(
            answ.proposals
                .into_iter()
                .map(|p| p.id)
                .collect::<Vec<u64>>(),
            [1, 2]
        );
        assert_eq!(answ.last_proposal_id, 2);

        let answ: FilterListProposalsResponse = app
            .wrap()
            .query_wasm_smart(
                govmod_single.clone(),
                &QueryMsg::FilterListProposals {
                    wallet: "one".into(),
                    status: None,
                    wallet_vote: WalletVote::AnyVote {},
                    start_after: Some(1),
                    limit: Some(10),
                },
            )
            .unwrap();
        assert_eq!(
            answ.proposals
                .into_iter()
                .map(|p| p.id)
                .collect::<Vec<u64>>(),
            [2, 3]
        );
        assert_eq!(answ.last_proposal_id, 3);

        // starting after {proposal_id: 2}, and limit 1
        // we should get 3rd, and not empty list
        let answ: FilterListProposalsResponse = app
            .wrap()
            .query_wasm_smart(
                govmod_single.clone(),
                &QueryMsg::FilterListProposals {
                    wallet: "one".into(),
                    status: None,
                    wallet_vote: WalletVote::Voted(MultipleChoiceVote { option_id: 0 }),
                    start_after: Some(1),
                    limit: Some(1),
                },
            )
            .unwrap();
        assert_eq!(
            answ.proposals
                .into_iter()
                .map(|p| p.id)
                .collect::<Vec<u64>>(),
            [3]
        );
        assert_eq!(answ.last_proposal_id, 3);

        let answ: FilterListProposalsResponse = app
            .wrap()
            .query_wasm_smart(
                govmod_single,
                &QueryMsg::FilterListProposals {
                    wallet: "one".into(),
                    status: None,
                    wallet_vote: WalletVote::AnyVote {},
                    start_after: Some(5),
                    limit: Some(5),
                },
            )
            .unwrap();
        assert_eq!(
            answ.proposals
                .into_iter()
                .map(|p| p.id)
                .collect::<Vec<u64>>(),
            [] as [u64; 0]
        );
        assert_eq!(answ.last_proposal_id, 0);
    }
}
