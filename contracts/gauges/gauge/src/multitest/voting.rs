use cosmwasm_std::{Addr, Decimal, Uint128};
use cw4::Member;
use cw_multi_test::Executor;
use dao_hooks::nft_stake::{NftStakeChangedExecuteMsg, NftStakeChangedHookMsg};
use dao_hooks::stake::StakeChangedExecuteMsg;
use dao_voting::voting::Vote;

use super::suite::SuiteBuilder;
use crate::error::ContractError;
use crate::msg::VoteInfo;

const EPOCH: u64 = 7 * 86_400;

#[test]
fn add_option() {
    let voter1 = "voter1";
    let voter2 = "voter2";
    let mut suite = SuiteBuilder::new()
        .with_voting_members(&[(voter1, 100), (voter2, 200)])
        .build();

    suite.next_block();
    suite
        .propose_update_proposal_module(voter1.to_string(), None)
        .unwrap();

    suite.next_block();
    let proposal = suite.list_proposals().unwrap()[0];
    suite
        .place_vote_single(voter1, proposal, Vote::Yes)
        .unwrap();
    suite
        .place_vote_single(voter2, proposal, Vote::Yes)
        .unwrap();

    suite.next_block();
    suite
        .execute_single_proposal(voter1.to_string(), proposal)
        .unwrap();
    let proposal_modules = suite.query_proposal_modules().unwrap();

    let gauge_contract = proposal_modules[1].clone();

    let gauge_adapter = suite
        .instantiate_adapter_and_create_gauge(
            gauge_contract.clone(),
            &[voter1, voter2],
            (1000, "ujuno"),
            None,
            None,
            None,
        )
        .unwrap();

    let gauge_id = 0; // first created gauge

    // gauge returns list all options; it does query adapter at initialization
    let options = suite.query_list_options(&gauge_contract, gauge_id).unwrap();
    assert_eq!(options.len(), 2);

    // add more valid options to gauge adapter
    suite
        .add_valid_option(&gauge_adapter, "addedoption1")
        .unwrap();
    suite
        .add_valid_option(&gauge_adapter, "addedoption2")
        .unwrap();

    // Voting members can add options
    suite
        .add_option(&gauge_contract, voter1, gauge_id, "addedoption1")
        .unwrap();
    suite
        .add_option(&gauge_contract, voter2, gauge_id, "addedoption2")
        .unwrap();
    let options = suite.query_list_options(&gauge_contract, gauge_id).unwrap();
    // added options are automatically voted for by creators
    assert_eq!(
        options,
        vec![
            ("addedoption1".to_owned(), Uint128::zero()),
            ("addedoption2".to_owned(), Uint128::zero()),
            ("voter1".to_owned(), Uint128::zero()),
            ("voter2".to_owned(), Uint128::zero())
        ]
    );

    // add another valid option to gauge adapter
    suite
        .add_valid_option(&gauge_adapter, "addedoption3")
        .unwrap();
    // Non-voting members cannot add options
    let err = suite
        .add_option(&gauge_contract, "random_voter", gauge_id, "addedoption3")
        .unwrap_err();
    assert_eq!(
        ContractError::NoVotingPower("random_voter".to_owned()),
        err.downcast().unwrap()
    );
}

#[test]
fn remove_option() {
    let voter1 = "voter1";
    let voter2 = "voter2";
    let mut suite = SuiteBuilder::new()
        .with_voting_members(&[(voter1, 100), (voter2, 200)])
        .build();

    suite.next_block();
    suite
        .propose_update_proposal_module(voter1.to_string(), None)
        .unwrap();
    let dao = suite.core.clone();
    suite.next_block();
    let proposal = suite.list_proposals().unwrap()[0];
    suite
        .place_vote_single(voter1, proposal, Vote::Yes)
        .unwrap();
    suite
        .place_vote_single(voter2, proposal, Vote::Yes)
        .unwrap();

    suite.next_block();
    suite
        .execute_single_proposal(voter1.to_string(), proposal)
        .unwrap();
    let proposal_modules = suite.query_proposal_modules().unwrap();
    let gauge_contract = proposal_modules[1].clone();

    let adapter = suite
        .instantiate_adapter_and_create_gauge(
            gauge_contract.clone(),
            &[voter1, voter2],
            (1000, "ujuno"),
            None,
            None,
            None,
        )
        .unwrap();

    let gauge_id = 0; // first created gauge

    // gauge returns list all options; it does query adapter at initialization
    let options = suite.query_list_options(&gauge_contract, gauge_id).unwrap();
    assert_eq!(
        options,
        vec![
            ("voter1".to_owned(), Uint128::zero()),
            ("voter2".to_owned(), Uint128::zero())
        ]
    );

    // add new valid options to the gauge adapter
    suite.add_valid_option(&adapter, "addedoption1").unwrap();
    suite.add_valid_option(&adapter, "addedoption2").unwrap();

    // Voting members can add options
    suite
        .add_option(&gauge_contract, voter1, gauge_id, "addedoption1")
        .unwrap();
    suite
        .add_option(&gauge_contract, voter2, gauge_id, "addedoption2")
        .unwrap();
    let options = suite.query_list_options(&gauge_contract, gauge_id).unwrap();
    // added options are automatically voted for by creators
    assert_eq!(
        options,
        vec![
            ("addedoption1".to_owned(), Uint128::zero()),
            ("addedoption2".to_owned(), Uint128::zero()),
            ("voter1".to_owned(), Uint128::zero()),
            ("voter2".to_owned(), Uint128::zero())
        ]
    );

    // owner can remove an option that has been added already
    suite
        .remove_option(&gauge_contract, dao.clone(), gauge_id, "addedoption1")
        .unwrap();

    // Anyone else cannot remove options
    let err = suite
        .remove_option(&gauge_contract, voter1, gauge_id, "addedoption2")
        .unwrap_err();

    assert_eq!(
        ContractError::Ownership(cw_ownable::OwnershipError::NotOwner),
        err.downcast().unwrap()
    );

    let options = suite.query_list_options(&gauge_contract, gauge_id).unwrap();
    // one has been removed
    assert_eq!(
        options,
        vec![
            ("addedoption2".to_owned(), Uint128::zero()),
            ("voter1".to_owned(), Uint128::zero()),
            ("voter2".to_owned(), Uint128::zero())
        ]
    );

    suite.invalidate_option(&adapter, "addedoption2").unwrap();

    // owner can remove an option that is no longer valid
    suite
        .remove_option(&gauge_contract, dao, gauge_id, "addedoption2")
        .unwrap();

    // Both options are now removed
    let options = suite.query_list_options(&gauge_contract, gauge_id).unwrap();
    assert_eq!(
        options,
        vec![
            ("voter1".to_owned(), Uint128::zero()),
            ("voter2".to_owned(), Uint128::zero())
        ]
    );
}

fn simple_vote(
    voter: &str,
    option: &str,
    percentage: u64,
    cast: impl Into<Option<u64>>,
) -> VoteInfo {
    VoteInfo {
        voter: voter.to_string(),
        votes: vec![crate::state::Vote {
            option: option.to_string(),
            weight: Decimal::percent(percentage),
        }],
        cast: cast.into(),
    }
}

fn multi_vote(voter: &str, votes: &[(&str, u64)], cast: impl Into<Option<u64>>) -> VoteInfo {
    let votes = votes
        .iter()
        .map(|(opt, percentage)| crate::state::Vote {
            option: opt.to_string(),
            weight: Decimal::percent(*percentage),
        })
        .collect();
    VoteInfo {
        voter: voter.to_string(),
        votes,
        cast: cast.into(),
    }
}

#[test]
fn vote_for_option() {
    let voter1 = "voter1";
    let voter2 = "voter2";
    let mut suite = SuiteBuilder::new()
        .with_voting_members(&[(voter1, 100), (voter2, 200)])
        .build();

    suite.next_block();
    suite
        .propose_update_proposal_module(voter1.to_string(), None)
        .unwrap();

    suite.next_block();
    let proposal = suite.list_proposals().unwrap()[0];
    suite
        .place_vote_single(voter1, proposal, Vote::Yes)
        .unwrap();
    suite
        .place_vote_single(voter2, proposal, Vote::Yes)
        .unwrap();

    suite.next_block();
    suite
        .execute_single_proposal(voter1.to_string(), proposal)
        .unwrap();
    let proposal_modules = suite.query_proposal_modules().unwrap();

    let gauge_contract = proposal_modules[1].clone();

    let gauge_adapter = suite
        .instantiate_adapter_and_create_gauge(
            gauge_contract.clone(),
            &[voter1, voter2],
            (1000, "ujuno"),
            None,
            None,
            None,
        )
        .unwrap();

    let gauge_id = 0; // first created gauge

    // vote for option from adapter (voting members are by default
    // options in adapter in this test suite)
    suite
        .place_votes(
            &gauge_contract,
            voter1.to_owned(),
            gauge_id,
            Some(vec![(voter1.to_owned(), Decimal::percent(90))]),
        )
        .unwrap();
    assert_eq!(
        simple_vote(voter1, voter1, 90, suite.current_time()),
        suite
            .query_vote(&gauge_contract, gauge_id, voter1)
            .unwrap()
            .unwrap(),
    );
    // check tally is proper
    let selected_set = suite.query_selected_set(&gauge_contract, gauge_id).unwrap();
    assert_eq!(selected_set, vec![(voter1.to_string(), Uint128::new(90))]);

    // add new valid options to the gauge adapter
    suite.add_valid_option(&gauge_adapter, "option1").unwrap();
    suite.add_valid_option(&gauge_adapter, "option2").unwrap();

    // change vote for option added through gauge
    suite
        .add_option(&gauge_contract, voter1, gauge_id, "option1")
        .unwrap();
    suite
        .add_option(&gauge_contract, voter1, gauge_id, "option2")
        .unwrap();
    // voter2 drops vote as well
    suite
        .place_votes(
            &gauge_contract,
            voter2.to_owned(),
            gauge_id,
            Some(vec![
                ("option1".to_owned(), Decimal::percent(50)),
                ("option2".to_owned(), Decimal::percent(50)),
            ]),
        )
        .unwrap();
    assert_eq!(
        vec![
            simple_vote(voter1, voter1, 90, suite.current_time()),
            multi_vote(
                voter2,
                &[("option1", 50), ("option2", 50)],
                suite.current_time()
            ),
        ],
        suite.query_list_votes(&gauge_contract, gauge_id).unwrap()
    );

    // placing vote again overwrites previous ones
    suite
        .place_votes(
            &gauge_contract,
            voter1.to_owned(),
            gauge_id,
            Some(vec![("option1".to_owned(), Decimal::percent(90))]),
        )
        .unwrap();
    suite
        .place_votes(
            &gauge_contract,
            voter2.to_owned(),
            gauge_id,
            Some(vec![("option1".to_owned(), Decimal::percent(90))]),
        )
        .unwrap();
    assert_eq!(
        vec![
            simple_vote(voter1, "option1", 90, suite.current_time()),
            simple_vote(voter2, "option1", 90, suite.current_time()),
        ],
        suite.query_list_votes(&gauge_contract, gauge_id).unwrap()
    );

    // vote for non-existing option
    let err = suite
        .place_vote(
            &gauge_contract,
            voter1.to_owned(),
            gauge_id,
            Some("random option".to_owned()),
        )
        .unwrap_err();
    assert_eq!(
        ContractError::OptionDoesNotExists {
            option: "random option".to_owned(),
            gauge_id
        },
        err.downcast().unwrap()
    );
}

#[test]
fn remove_vote() {
    let voter1 = "voter1";
    let voter2 = "voter2";
    let mut suite = SuiteBuilder::new()
        .with_voting_members(&[(voter1, 100), (voter2, 200)])
        .build();

    suite.next_block();
    suite
        .propose_update_proposal_module(voter1.to_string(), None)
        .unwrap();

    suite.next_block();
    let proposal = suite.list_proposals().unwrap()[0];
    suite
        .place_vote_single(voter1, proposal, Vote::Yes)
        .unwrap();
    suite
        .place_vote_single(voter2, proposal, Vote::Yes)
        .unwrap();

    suite.next_block();
    suite
        .execute_single_proposal(voter1.to_string(), proposal)
        .unwrap();
    let proposal_modules = suite.query_proposal_modules().unwrap();

    let gauge_contract = proposal_modules[1].clone();

    suite
        .instantiate_adapter_and_create_gauge(
            gauge_contract.clone(),
            &[voter1, voter2],
            (1000, "ujuno"),
            None,
            None,
            None,
        )
        .unwrap();

    let gauge_id = 0; // first created gauge

    // vote for option from adapter (voting members are by default
    // options in adapter in this test suite)
    suite
        .place_vote(
            &gauge_contract,
            voter1.to_owned(),
            gauge_id,
            Some(voter1.to_owned()),
        )
        .unwrap();
    suite
        .place_vote(
            &gauge_contract,
            voter2.to_owned(),
            gauge_id,
            Some(voter1.to_owned()),
        )
        .unwrap();
    assert_eq!(
        vec![
            simple_vote(voter1, voter1, 100, suite.current_time()),
            simple_vote(voter2, voter1, 100, suite.current_time()),
        ],
        suite.query_list_votes(&gauge_contract, gauge_id).unwrap()
    );

    // remove vote
    suite
        .place_vote(&gauge_contract, voter1.to_owned(), gauge_id, None)
        .unwrap();
    assert_eq!(
        vec![simple_vote(voter2, voter1, 100, suite.current_time())],
        suite.query_list_votes(&gauge_contract, gauge_id).unwrap()
    );
    assert_eq!(
        suite.query_vote(&gauge_contract, gauge_id, voter1).unwrap(),
        None
    );
    assert_eq!(
        suite.query_vote(&gauge_contract, gauge_id, voter2).unwrap(),
        Some(simple_vote(voter2, voter1, 100, suite.current_time())),
    );

    // remove nonexisting vote
    let err = suite
        .place_vote(&gauge_contract, voter1.to_owned(), gauge_id, None)
        .unwrap_err();
    assert_eq!(
        ContractError::CannotRemoveNonexistingVote {},
        err.downcast().unwrap()
    );
}

#[test]
fn votes_stays_the_same_after_execution() {
    let voter1 = "voter1";
    let voter2 = "voter2";
    let reward_to_distribute = (1000, "ujuno");
    let mut suite = SuiteBuilder::new()
        .with_voting_members(&[(voter1, 100), (voter2, 100)])
        .with_core_balance(reward_to_distribute)
        .build();

    suite.next_block();
    let gauge_config = suite
        .instantiate_adapter_and_return_config(
            &[voter1, voter2],
            reward_to_distribute,
            None,
            None,
            None,
        )
        .unwrap();
    suite
        .propose_update_proposal_module(voter1.to_string(), vec![gauge_config])
        .unwrap();

    suite.next_block();
    let proposal = suite.list_proposals().unwrap()[0];
    suite
        .place_vote_single(voter1, proposal, Vote::Yes)
        .unwrap();
    suite
        .place_vote_single(voter2, proposal, Vote::Yes)
        .unwrap();

    suite.next_block();
    suite
        .execute_single_proposal(voter1.to_string(), proposal)
        .unwrap();
    let proposal_modules = suite.query_proposal_modules().unwrap();
    let gauge_contract = proposal_modules[1].clone();

    let gauge_id = 0;

    // vote for one of the options in gauge
    suite
        .place_vote(
            &gauge_contract,
            voter1.to_owned(),
            gauge_id,
            Some(voter1.to_owned()), // option to vote for
        )
        .unwrap();
    suite
        .place_vote(
            &gauge_contract,
            voter2.to_owned(),
            gauge_id,
            Some(voter1.to_owned()),
        )
        .unwrap();

    let selected_set = suite.query_selected_set(&gauge_contract, gauge_id).unwrap();
    // voter1 was option voted for with two 100 voting powers combined
    assert_eq!(selected_set, vec![("voter1".to_owned(), Uint128::new(200))]);

    // before advancing specified epoch tally won't get sampled
    suite.advance_time(EPOCH);

    assert_eq!(
        vec![
            simple_vote(voter1, voter1, 100, suite.current_time() - EPOCH),
            simple_vote(voter2, voter1, 100, suite.current_time() - EPOCH)
        ],
        suite.query_list_votes(&gauge_contract, gauge_id).unwrap()
    );
    suite
        .execute_options(&gauge_contract, voter1, gauge_id)
        .unwrap();

    assert_eq!(
        vec![
            simple_vote(voter1, voter1, 100, suite.current_time() - EPOCH),
            simple_vote(voter2, voter1, 100, suite.current_time() - EPOCH)
        ],
        suite.query_list_votes(&gauge_contract, gauge_id).unwrap()
    );
    assert_eq!(
        suite.query_vote(&gauge_contract, gauge_id, voter1).unwrap(),
        Some(simple_vote(
            voter1,
            voter1,
            100,
            suite.current_time() - EPOCH
        )),
    );
    assert_eq!(
        suite.query_vote(&gauge_contract, gauge_id, voter2).unwrap(),
        Some(simple_vote(
            voter2,
            voter1,
            100,
            suite.current_time() - EPOCH
        )),
    );
}

#[test]
fn vote_for_max_capped_option() {
    let voter1 = "voter1";
    let voter2 = "voter2";
    let mut suite = SuiteBuilder::new()
        .with_voting_members(&[(voter1, 100), (voter2, 100)])
        .build();

    suite.next_block();
    suite
        .propose_update_proposal_module(voter1.to_string(), None)
        .unwrap();

    suite.next_block();
    let proposal = suite.list_proposals().unwrap()[0];
    suite
        .place_vote_single(voter1, proposal, Vote::Yes)
        .unwrap();
    suite
        .place_vote_single(voter2, proposal, Vote::Yes)
        .unwrap();

    suite.next_block();
    suite
        .execute_single_proposal(voter1.to_string(), proposal)
        .unwrap();
    let proposal_modules = suite.query_proposal_modules().unwrap();

    let gauge_contract = proposal_modules[1].clone();

    let gauge_adapter = suite
        .instantiate_adapter_and_create_gauge(
            gauge_contract.clone(),
            &[voter1, voter2],
            (1000, "ujuno"),
            Some(Decimal::percent(10)),
            None,
            None,
        )
        .unwrap();

    let gauge_id = 0; // first created gauge

    // wait until epoch passes
    suite.advance_time(EPOCH);

    // add more valid options to gauge adapter
    suite.add_valid_option(&gauge_adapter, "option1").unwrap();
    suite.add_valid_option(&gauge_adapter, "option2").unwrap();

    // change vote for option added through gauge
    suite
        .add_option(&gauge_contract, voter1, gauge_id, "option1")
        .unwrap();
    suite
        .add_option(&gauge_contract, voter1, gauge_id, "option2")
        .unwrap();

    // vote 100% voting power on 'voter1' option (100 weight)
    suite
        .place_vote(
            &gauge_contract,
            voter1,
            gauge_id,
            Some("option1".to_owned()),
        )
        .unwrap();
    // vote 10% voting power on 'voter2' option (10 weight)
    suite
        .place_votes(
            &gauge_contract,
            voter2,
            gauge_id,
            vec![("option2".to_owned(), Decimal::percent(10))],
        )
        .unwrap();

    assert_eq!(
        vec![
            multi_vote(voter1, &[("option1", 100)], suite.current_time()),
            multi_vote(voter2, &[("option2", 10)], suite.current_time()),
        ],
        suite.query_list_votes(&gauge_contract, gauge_id).unwrap()
    );

    let selected_set = suite.query_selected_set(&gauge_contract, gauge_id).unwrap();
    // Despite 'option1' having 100 voting power and option2 having 10 voting power,
    // because of max vote cap set to 10% now 'option1' will have its power decreased to 10% * 110
    // 'option2' stays at 10 voting power as it was below 10% of total votes
    assert_eq!(
        selected_set,
        vec![
            ("option1".to_owned(), Uint128::new(11)),
            ("option2".to_owned(), Uint128::new(10))
        ]
    );
}

#[test]
fn membership_voting_power_change() {
    let voter1 = "voter1";
    let voter2 = "voter2";
    let mut suite = SuiteBuilder::new()
        .with_voting_members(&[(voter1, 100), (voter2, 200)])
        .with_core_balance((10000, "ujuno"))
        .build();

    suite.next_block();
    suite
        .propose_update_proposal_module(voter1.to_string(), None)
        .unwrap();

    suite.next_block();
    let proposal = suite.list_proposals().unwrap()[0];
    suite
        .place_vote_single(voter1, proposal, Vote::Yes)
        .unwrap();
    suite
        .place_vote_single(voter2, proposal, Vote::Yes)
        .unwrap();

    suite.next_block();
    suite
        .execute_single_proposal(voter1.to_string(), proposal)
        .unwrap();
    let proposal_modules = suite.query_proposal_modules().unwrap();

    let gauge_contract = proposal_modules[1].clone();

    // Setup membership change hooks
    suite
        .propose_add_membership_change_hook(voter1.to_string(), gauge_contract.clone())
        .unwrap();
    let proposal = suite.list_proposals().unwrap()[1];
    suite
        .place_vote_single(voter1, proposal, Vote::Yes)
        .unwrap();
    suite
        .place_vote_single(voter2, proposal, Vote::Yes)
        .unwrap();

    suite.next_block();
    suite
        .execute_single_proposal(voter1.to_string(), proposal)
        .unwrap();

    let gauge_adapter = suite
        .instantiate_adapter_and_create_gauge(
            gauge_contract.clone(),
            &[voter1, voter2],
            (1000, "ujuno"),
            None,
            None,
            None,
        )
        .unwrap();
    let gauge_id = 0; // first created gauge

    // vote for option from adapter (voting members are by default
    // options in adapter in this test suite)
    suite
        .place_votes(
            &gauge_contract,
            voter1.to_owned(),
            gauge_id,
            Some(vec![(voter1.to_owned(), Decimal::percent(90))]),
        )
        .unwrap();
    assert_eq!(
        simple_vote(voter1, voter1, 90, suite.current_time()),
        suite
            .query_vote(&gauge_contract, gauge_id, voter1)
            .unwrap()
            .unwrap(),
    );
    // check tally is proper
    let selected_set = suite.query_selected_set(&gauge_contract, gauge_id).unwrap();
    assert_eq!(selected_set, vec![(voter1.to_string(), Uint128::new(90))]);

    // add new valid options to the gauge adapter
    suite.add_valid_option(&gauge_adapter, "option1").unwrap();
    suite.add_valid_option(&gauge_adapter, "option2").unwrap();

    // change vote for option added through gauge
    suite
        .add_option(&gauge_contract, voter1, gauge_id, "option1")
        .unwrap();
    suite
        .add_option(&gauge_contract, voter1, gauge_id, "option2")
        .unwrap();
    // voter2 drops vote as well
    suite
        .place_votes(
            &gauge_contract,
            voter2.to_owned(),
            gauge_id,
            Some(vec![
                ("option1".to_owned(), Decimal::percent(50)),
                ("option2".to_owned(), Decimal::percent(50)),
            ]),
        )
        .unwrap();
    assert_eq!(
        vec![
            simple_vote(voter1, voter1, 90, suite.current_time()),
            multi_vote(
                voter2,
                &[("option1", 50), ("option2", 50)],
                suite.current_time()
            ),
        ],
        suite.query_list_votes(&gauge_contract, gauge_id).unwrap()
    );

    // Execute after epoch passes
    suite.advance_time(EPOCH);
    suite
        .execute_options(&gauge_contract, voter1, gauge_id)
        .unwrap();

    let pre_voter1_takeover_gauge_set =
        suite.query_selected_set(&gauge_contract, gauge_id).unwrap();

    // Voter one's option is least popular
    assert_eq!(
        pre_voter1_takeover_gauge_set,
        vec![
            ("option2".to_string(), Uint128::new(100)),
            ("option1".to_string(), Uint128::new(100)),
            ("voter1".to_string(), Uint128::new(90))
        ]
    );

    // Force update members, giving voter 1 more power
    suite
        .force_update_members(
            vec![],
            vec![Member {
                addr: voter1.to_string(),
                weight: 1000,
            }],
        )
        .unwrap();
    suite.next_block();

    let current_gauge_set = suite.query_selected_set(&gauge_contract, gauge_id).unwrap();

    // Currect selected set should be different than before voter1 got power
    assert_ne!(pre_voter1_takeover_gauge_set, current_gauge_set);

    // Voter1 option is now most popular
    assert_eq!(
        current_gauge_set,
        vec![
            ("voter1".to_string(), Uint128::new(900)),
            ("option2".to_string(), Uint128::new(100)),
            ("option1".to_string(), Uint128::new(100))
        ]
    );

    // Execute after epoch passes
    suite.advance_time(EPOCH);
    suite
        .execute_options(&gauge_contract, voter1, gauge_id)
        .unwrap();

    // Force update members, kick out voter 1
    suite
        .force_update_members(vec![voter1.to_string()], vec![])
        .unwrap();
    suite.next_block();

    // Execute after epoch passes
    suite.advance_time(EPOCH);
    suite
        .execute_options(&gauge_contract, voter1, gauge_id)
        .unwrap();

    let current_gauge_set = suite
        .query_last_executed_set(&gauge_contract, gauge_id)
        .unwrap();

    // Voter1 removed and so is the one thing they voted for
    assert_eq!(
        current_gauge_set,
        Some(vec![
            ("option2".to_string(), Uint128::new(100)),
            ("option1".to_string(), Uint128::new(100))
        ])
    );
}

#[test]
fn token_staking_voting_power_change() {
    let voter1 = "voter1";
    let voter2 = "voter2";
    let hook_caller = "token-staking-contract";
    let mut suite = SuiteBuilder::new()
        .with_voting_members(&[(voter1, 100), (voter2, 200)])
        .with_core_balance((10000, "ujuno"))
        .build();

    suite.next_block();
    suite
        .propose_update_proposal_module_custom_hook_caller(
            voter1.to_string(),
            hook_caller.to_string(),
            None,
        )
        .unwrap();

    suite.next_block();
    let proposal = suite.list_proposals().unwrap()[0];
    suite
        .place_vote_single(voter1, proposal, Vote::Yes)
        .unwrap();
    suite
        .place_vote_single(voter2, proposal, Vote::Yes)
        .unwrap();

    suite.next_block();
    suite
        .execute_single_proposal(voter1.to_string(), proposal)
        .unwrap();
    let proposal_modules = suite.query_proposal_modules().unwrap();

    let gauge_contract = proposal_modules[1].clone();

    let gauge_adapter = suite
        .instantiate_adapter_and_create_gauge(
            gauge_contract.clone(),
            &[voter1, voter2],
            (1000, "ujuno"),
            None,
            None,
            None,
        )
        .unwrap();
    let gauge_id = 0; // first created gauge

    // vote for option from adapter (voting members are by default
    // options in adapter in this test suite)
    suite
        .place_votes(
            &gauge_contract,
            voter1.to_owned(),
            gauge_id,
            Some(vec![(voter1.to_owned(), Decimal::percent(90))]),
        )
        .unwrap();
    assert_eq!(
        simple_vote(voter1, voter1, 90, suite.current_time()),
        suite
            .query_vote(&gauge_contract, gauge_id, voter1)
            .unwrap()
            .unwrap(),
    );
    // check tally is proper
    let selected_set = suite.query_selected_set(&gauge_contract, gauge_id).unwrap();
    assert_eq!(selected_set, vec![(voter1.to_string(), Uint128::new(90))]);

    // add new valid options to the gauge adapter
    suite.add_valid_option(&gauge_adapter, "option1").unwrap();
    suite.add_valid_option(&gauge_adapter, "option2").unwrap();

    // change vote for option added through gauge
    suite
        .add_option(&gauge_contract, voter1, gauge_id, "option1")
        .unwrap();
    suite
        .add_option(&gauge_contract, voter1, gauge_id, "option2")
        .unwrap();
    // voter2 drops vote as well
    suite
        .place_votes(
            &gauge_contract,
            voter2.to_owned(),
            gauge_id,
            Some(vec![
                ("option1".to_owned(), Decimal::percent(50)),
                ("option2".to_owned(), Decimal::percent(50)),
            ]),
        )
        .unwrap();
    assert_eq!(
        vec![
            simple_vote(voter1, voter1, 90, suite.current_time()),
            multi_vote(
                voter2,
                &[("option1", 50), ("option2", 50)],
                suite.current_time()
            ),
        ],
        suite.query_list_votes(&gauge_contract, gauge_id).unwrap()
    );

    // Execute after epoch passes
    suite.advance_time(EPOCH);
    suite
        .execute_options(&gauge_contract, voter1, gauge_id)
        .unwrap();

    let pre_voter1_takeover_gauge_set =
        suite.query_selected_set(&gauge_contract, gauge_id).unwrap();

    // Voter one's option is least popular
    assert_eq!(
        pre_voter1_takeover_gauge_set,
        vec![
            ("option2".to_string(), Uint128::new(100)),
            ("option1".to_string(), Uint128::new(100)),
            ("voter1".to_string(), Uint128::new(90))
        ]
    );

    // Use hook caller to mock voter1 staking
    suite
        .app
        .execute_contract(
            Addr::unchecked(hook_caller),
            gauge_contract.clone(),
            &StakeChangedExecuteMsg::StakeChangeHook(
                dao_hooks::stake::StakeChangedHookMsg::Stake {
                    addr: Addr::unchecked(voter1),
                    amount: Uint128::new(900),
                },
            ),
            &[],
        )
        .unwrap();

    suite.next_block();

    let current_gauge_set = suite.query_selected_set(&gauge_contract, gauge_id).unwrap();

    // Currect selected set should be different than before voter1 got power
    assert_ne!(pre_voter1_takeover_gauge_set, current_gauge_set);

    // Voter1 option is now most popular
    assert_eq!(
        current_gauge_set,
        vec![
            ("voter1".to_string(), Uint128::new(900)),
            ("option2".to_string(), Uint128::new(100)),
            ("option1".to_string(), Uint128::new(100))
        ]
    );

    // Execute after epoch passes
    suite.advance_time(EPOCH);
    suite
        .execute_options(&gauge_contract, voter1, gauge_id)
        .unwrap();

    // Mock voter 1 unstaking
    suite
        .app
        .execute_contract(
            Addr::unchecked(hook_caller),
            gauge_contract.clone(),
            &StakeChangedExecuteMsg::StakeChangeHook(
                dao_hooks::stake::StakeChangedHookMsg::Unstake {
                    addr: Addr::unchecked(voter1),
                    amount: Uint128::new(1000),
                },
            ),
            &[],
        )
        .unwrap();
    suite.next_block();

    // Execute after epoch passes
    suite.advance_time(EPOCH);
    suite
        .execute_options(&gauge_contract, voter1, gauge_id)
        .unwrap();

    let current_gauge_set = suite
        .query_last_executed_set(&gauge_contract, gauge_id)
        .unwrap();

    // Voter1 removed and so is the one thing they voted for
    assert_eq!(
        current_gauge_set,
        Some(vec![
            ("option2".to_string(), Uint128::new(100)),
            ("option1".to_string(), Uint128::new(100))
        ])
    );
}

#[test]
fn nft_staking_voting_power_change() {
    let voter1 = "voter1";
    let voter2 = "voter2";
    let hook_caller = "nft-staking-contract";
    let mut suite = SuiteBuilder::new()
        .with_voting_members(&[(voter1, 1), (voter2, 2)])
        .with_core_balance((10000, "ujuno"))
        .build();

    suite.next_block();
    suite
        .propose_update_proposal_module_custom_hook_caller(
            voter1.to_string(),
            hook_caller.to_string(),
            None,
        )
        .unwrap();

    suite.next_block();
    let proposal = suite.list_proposals().unwrap()[0];
    suite
        .place_vote_single(voter1, proposal, Vote::Yes)
        .unwrap();
    suite
        .place_vote_single(voter2, proposal, Vote::Yes)
        .unwrap();

    suite.next_block();
    suite
        .execute_single_proposal(voter1.to_string(), proposal)
        .unwrap();
    let proposal_modules = suite.query_proposal_modules().unwrap();

    let gauge_contract = proposal_modules[1].clone();

    let gauge_adapter = suite
        .instantiate_adapter_and_create_gauge(
            gauge_contract.clone(),
            &[voter1, voter2],
            (1000, "ujuno"),
            None,
            None,
            None,
        )
        .unwrap();
    let gauge_id = 0; // first created gauge

    // vote for option from adapter (voting members are by default
    // options in adapter in this test suite)
    suite
        .place_votes(
            &gauge_contract,
            voter1.to_owned(),
            gauge_id,
            Some(vec![(voter1.to_owned(), Decimal::percent(100))]),
        )
        .unwrap();
    assert_eq!(
        simple_vote(voter1, voter1, 100, suite.current_time()),
        suite
            .query_vote(&gauge_contract, gauge_id, voter1)
            .unwrap()
            .unwrap(),
    );
    // check tally is proper
    let selected_set = suite.query_selected_set(&gauge_contract, gauge_id).unwrap();
    assert_eq!(selected_set, vec![(voter1.to_string(), Uint128::one())]);

    // add new valid options to the gauge adapter
    suite.add_valid_option(&gauge_adapter, "option1").unwrap();
    suite.add_valid_option(&gauge_adapter, "option2").unwrap();

    // change vote for option added through gauge
    suite
        .add_option(&gauge_contract, voter1, gauge_id, "option1")
        .unwrap();
    suite
        .add_option(&gauge_contract, voter1, gauge_id, "option2")
        .unwrap();
    // voter2 drops vote as well
    suite
        .place_votes(
            &gauge_contract,
            voter2.to_owned(),
            gauge_id,
            Some(vec![
                ("option1".to_owned(), Decimal::percent(50)),
                ("option2".to_owned(), Decimal::percent(50)),
            ]),
        )
        .unwrap();
    assert_eq!(
        vec![
            simple_vote(voter1, voter1, 100, suite.current_time()),
            multi_vote(
                voter2,
                &[("option1", 50), ("option2", 50)],
                suite.current_time()
            ),
        ],
        suite.query_list_votes(&gauge_contract, gauge_id).unwrap()
    );

    // Execute after epoch passes
    suite.advance_time(EPOCH);
    suite
        .execute_options(&gauge_contract, voter1, gauge_id)
        .unwrap();

    let pre_voter1_takeover_gauge_set =
        suite.query_selected_set(&gauge_contract, gauge_id).unwrap();

    // Voter one's option is least popular
    assert_eq!(
        pre_voter1_takeover_gauge_set,
        vec![
            ("voter1".to_string(), Uint128::new(1)),
            ("option2".to_string(), Uint128::new(1)),
            ("option1".to_string(), Uint128::new(1)),
        ]
    );

    // Mock voter 1 staking NFT
    suite
        .app
        .execute_contract(
            Addr::unchecked(hook_caller),
            gauge_contract.clone(),
            &NftStakeChangedExecuteMsg::NftStakeChangeHook(NftStakeChangedHookMsg::Stake {
                addr: Addr::unchecked(voter1),
                token_id: "1".to_string(),
            }),
            &[],
        )
        .unwrap();

    suite.next_block();

    let current_gauge_set = suite.query_selected_set(&gauge_contract, gauge_id).unwrap();

    // Currect selected set should be different than before voter1 got power
    assert_ne!(pre_voter1_takeover_gauge_set, current_gauge_set);

    // Voter1 option is now most popular
    assert_eq!(
        current_gauge_set,
        vec![
            ("voter1".to_string(), Uint128::new(2)),
            ("option2".to_string(), Uint128::new(1)),
            ("option1".to_string(), Uint128::new(1))
        ]
    );

    // Execute after epoch passes
    suite.advance_time(EPOCH);
    suite
        .execute_options(&gauge_contract, voter1, gauge_id)
        .unwrap();

    // Mock voter1 unstaking 2 nfts
    suite
        .app
        .execute_contract(
            Addr::unchecked(hook_caller),
            gauge_contract.clone(),
            &NftStakeChangedExecuteMsg::NftStakeChangeHook(NftStakeChangedHookMsg::Unstake {
                addr: Addr::unchecked(voter1),
                token_ids: vec!["1".to_string(), "2".to_string()],
            }),
            &[],
        )
        .unwrap();
    suite.next_block();

    // Execute after epoch passes
    suite.advance_time(EPOCH);
    suite
        .execute_options(&gauge_contract, voter1, gauge_id)
        .unwrap();

    let current_gauge_set = suite
        .query_last_executed_set(&gauge_contract, gauge_id)
        .unwrap();

    // Voter1 removed and so is the one thing they voted for
    assert_eq!(
        current_gauge_set,
        Some(vec![
            ("option2".to_string(), Uint128::new(1)),
            ("option1".to_string(), Uint128::new(1))
        ])
    );
}
