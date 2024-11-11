use cosmwasm_std::Uint128;
use dao_voting::voting::Vote;

use super::suite::SuiteBuilder;

#[test]
fn multiple_options_one_gauge() {
    let voter1 = "voter1";
    let voter2 = "voter2";
    let voter3 = "voter3";
    let voter4 = "voter4";
    let voter5 = "voter5";
    let reward_to_distribute = (1000, "ujuno");
    let mut suite = SuiteBuilder::new()
        .with_voting_members(&[
            (voter1, 600), // to have majority...
            (voter2, 120),
            (voter3, 130),
            (voter4, 140),
            (voter5, 150),
        ])
        .with_core_balance(reward_to_distribute)
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

    suite.next_block();
    suite
        .execute_single_proposal(voter1.to_string(), proposal)
        .unwrap();
    let proposal_modules = suite.query_proposal_modules().unwrap();
    let gauge_contract = proposal_modules[1].clone();

    suite
        .instantiate_adapter_and_create_gauge(
            gauge_contract.clone(),
            &["option1", "option2", "option3", "option4", "option5"],
            reward_to_distribute,
            None,
            None,
            None,
        )
        .unwrap();
    let gauge_id = 0;

    suite
        .place_vote(
            &gauge_contract,
            voter1.to_owned(),
            gauge_id,
            Some("option1".into()),
        )
        .unwrap();
    suite
        .place_vote(
            &gauge_contract,
            voter2.to_owned(),
            gauge_id,
            Some("option2".into()),
        )
        .unwrap();
    suite
        .place_vote(
            &gauge_contract,
            voter3.to_owned(),
            gauge_id,
            Some("option3".into()),
        )
        .unwrap();
    suite
        .place_vote(
            &gauge_contract,
            voter4.to_owned(),
            gauge_id,
            Some("option4".into()),
        )
        .unwrap();
    suite
        .place_vote(
            &gauge_contract,
            voter5.to_owned(),
            gauge_id,
            Some("option5".into()),
        )
        .unwrap();

    let selected_set = suite.query_selected_set(&gauge_contract, gauge_id).unwrap();
    assert_eq!(
        selected_set,
        vec![
            ("option1".to_owned(), Uint128::new(600)),
            ("option5".to_owned(), Uint128::new(150)),
            ("option4".to_owned(), Uint128::new(140)),
            ("option3".to_owned(), Uint128::new(130)),
            ("option2".to_owned(), Uint128::new(120))
        ]
    );

    suite
        .place_vote(
            &gauge_contract,
            voter1.to_owned(),
            gauge_id,
            Some("option2".into()),
        )
        .unwrap();

    let selected_set = suite.query_selected_set(&gauge_contract, gauge_id).unwrap();
    assert_eq!(
        selected_set,
        vec![
            ("option2".to_owned(), Uint128::new(720)),
            ("option5".to_owned(), Uint128::new(150)),
            ("option4".to_owned(), Uint128::new(140)),
            ("option3".to_owned(), Uint128::new(130)),
        ]
    );
}

/// create one in instantiate, other later via create
#[test]
fn multiple_options_two_gauges() {
    let voter1 = "voter1";
    let voter2 = "voter2";
    let voter3 = "voter3";
    let voter4 = "voter4";
    let voter5 = "voter5";
    let reward_to_distribute = (1000, "ujuno");
    let mut suite = SuiteBuilder::new()
        .with_voting_members(&[
            (voter1, 600), // to have majority
            (voter2, 120),
            (voter3, 130),
            (voter4, 140),
            (voter5, 150),
        ])
        .with_core_balance(reward_to_distribute)
        .build();

    suite.next_block();
    let gauge_config = suite
        .instantiate_adapter_and_return_config(
            &["option1", "option2"],
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

    suite.next_block();
    suite
        .execute_single_proposal(voter1.to_string(), proposal)
        .unwrap();
    let proposal_modules = suite.query_proposal_modules().unwrap();
    let gauge_contract = proposal_modules[1].clone();

    let first_gauge_id = 0;
    suite
        .instantiate_adapter_and_create_gauge(
            gauge_contract.clone(),
            &["option3", "option4", "option5"],
            reward_to_distribute,
            None,
            None,
            None,
        )
        .unwrap();
    let second_gauge_id = 1;

    suite
        .place_vote(
            &gauge_contract,
            voter1.to_owned(),
            first_gauge_id,
            Some("option2".into()),
        )
        .unwrap();
    suite
        .place_vote(
            &gauge_contract,
            voter2.to_owned(),
            first_gauge_id,
            Some("option2".into()),
        )
        .unwrap();
    suite
        .place_vote(
            &gauge_contract,
            voter3.to_owned(),
            second_gauge_id,
            Some("option3".into()),
        )
        .unwrap();
    suite
        .place_vote(
            &gauge_contract,
            voter4.to_owned(),
            second_gauge_id,
            Some("option5".into()),
        )
        .unwrap();
    suite
        .place_vote(
            &gauge_contract,
            voter5.to_owned(),
            second_gauge_id,
            Some("option5".into()),
        )
        .unwrap();

    let selected_set = suite
        .query_selected_set(&gauge_contract, first_gauge_id)
        .unwrap();
    assert_eq!(
        selected_set,
        vec![("option2".to_owned(), Uint128::new(720))]
    );

    let selected_set = suite
        .query_selected_set(&gauge_contract, second_gauge_id)
        .unwrap();
    assert_eq!(
        selected_set,
        vec![
            ("option5".to_owned(), Uint128::new(290)),
            ("option3".to_owned(), Uint128::new(130)),
        ]
    );
}

#[test]
fn not_voted_options_are_not_selected() {
    let voter1 = "voter1";
    let voter2 = "voter2";
    let reward_to_distribute = (1000, "ujuno");
    let mut suite = SuiteBuilder::new()
        .with_voting_members(&[
            (voter1, 600), // to have majority
            (voter2, 120),
        ])
        .with_core_balance(reward_to_distribute)
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

    suite.next_block();
    suite
        .execute_single_proposal(voter1.to_string(), proposal)
        .unwrap();
    let proposal_modules = suite.query_proposal_modules().unwrap();
    let gauge_contract = proposal_modules[1].clone();

    suite
        .instantiate_adapter_and_create_gauge(
            gauge_contract.clone(),
            &["option1", "option2", "option3", "option4"],
            reward_to_distribute,
            None,
            None,
            None,
        )
        .unwrap();
    let first_gauge_id = 0;

    suite
        .place_vote(
            &gauge_contract,
            voter1.to_owned(),
            first_gauge_id,
            Some("option1".into()),
        )
        .unwrap();
    suite
        .place_vote(
            &gauge_contract,
            voter2.to_owned(),
            first_gauge_id,
            Some("option2".into()),
        )
        .unwrap();

    let selected_set = suite
        .query_selected_set(&gauge_contract, first_gauge_id)
        .unwrap();
    assert_eq!(
        selected_set,
        vec![
            ("option1".to_owned(), Uint128::new(600)),
            ("option2".to_owned(), Uint128::new(120)),
        ]
    );

    // first voter changes vote to option2
    suite
        .place_vote(
            &gauge_contract,
            voter1.to_owned(),
            first_gauge_id,
            Some("option2".into()),
        )
        .unwrap();
    let selected_set = suite
        .query_selected_set(&gauge_contract, first_gauge_id)
        .unwrap();
    assert_eq!(
        selected_set,
        vec![("option2".to_owned(), Uint128::new(720)),]
    );
}
